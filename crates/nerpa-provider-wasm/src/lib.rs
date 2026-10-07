//! Third-party providers as WASM components, admitted over a WIT interface.
//!
//! A provider written outside this repository ships as a `.wasm` component and
//! a manifest beside it. The manifest is data — the provider's name and what its
//! kinds own — and the component is the logic. This crate loads both and turns
//! them into a [`Provider`] that registers beside the in-tree ones
//! (adr-2026-0004).
//!
//! The first step is the loading: a component is admitted and its declared kinds
//! enter the schema a configuration can name. The operations that read and
//! change the world arrive with the WIT interface itself, in later work.

mod convert;
mod manifest;

use std::path::Path;

mod bindings {
    wasmtime::component::bindgen!({
        path: "wit",
        world: "provider",
    });
}

use nerpa_core::Schema;
use nerpa_core::address::{Address, Kind, Name, Provider};
use nerpa_core::observed::Facts;
use nerpa_core::provider::{
    Afterwards, Answered, Concluded, Concluding, Planned, Prepared, Provider as ProviderTrait,
    ProviderError, Reach, Read, Reading, Sending,
};
use wasmtime::component::{Component, HasSelf, Linker};
use wasmtime::{Engine, Store};

pub use manifest::Manifest;

// The machine a batch is read from, for the duration of one synchronous `read`.
// wasmtime's store data must be `'static`, and a borrowed `Reach` is not, so the
// component's `reach` import resolves it here instead of through the store. The
// cell holds a thin pointer to the `&dyn Reach` value, which lives on the stack
// of `with_reach` for the whole call.
thread_local! {
    static REACH: std::cell::Cell<*const ()> = const { std::cell::Cell::new(std::ptr::null()) };
}

/// Restores the previous reach pointer when the scoped read ends.
struct Reset<'a>(&'a std::cell::Cell<*const ()>, *const ());

impl Drop for Reset<'_> {
    fn drop(&mut self) {
        self.0.set(self.1);
    }
}

/// Sets `reach` for the duration of `run`, restoring what was there before.
fn with_reach<T>(reach: &dyn Reach, run: impl FnOnce() -> T) -> T {
    REACH.with(|cell| {
        let previous = cell.replace(std::ptr::from_ref(&reach).cast::<()>());
        let _reset = Reset(cell, previous);
        run()
    })
}

/// The reach currently set by [`with_reach`].
#[allow(
    unsafe_code,
    reason = "the pointer is set by `with_reach` for one synchronous read call and cleared before the borrow ends"
)]
fn current_reach() -> &'static dyn Reach {
    REACH.with(|cell| {
        let ptr = cell.get();
        assert!(!ptr.is_null(), "the WASM provider reads with no reach set");
        unsafe { *ptr.cast::<&dyn Reach>() }
    })
}

/// The host state behind the component's imports. It holds the manifest's
/// credential and the hosts the component may reach, and tracks the credential
/// handles it has handed out so `request` can resolve one back to the value the
/// component never sees.
struct Host {
    /// The credential value the host alone holds; `None` when the manifest
    /// declares none.
    token: Option<String>,
    /// The hosts the component's requests may reach.
    hosts: Vec<String>,
    /// The value behind each credential handle, indexed by the handle's rep.
    /// A spent handle is `None`.
    credentials: Vec<Option<String>>,
}

impl Host {
    /// The host state for one instantiation.
    fn new(token: Option<String>, hosts: Vec<String>) -> Self {
        Self {
            token,
            hosts,
            credentials: Vec::new(),
        }
    }
}

impl bindings::ProviderImports for Host {
    fn reach(&mut self, question: bindings::Ask) -> Result<bindings::Answer, String> {
        let ask = convert::ask_from_wit(&question)?;
        let answer = current_reach()
            .ask(&ask)
            .map_err(|error| error.to_string())?;
        Ok(convert::answer_to_wit(&answer))
    }

    fn credential_handle(&mut self) -> wasmtime::component::Resource<bindings::Credential> {
        let rep = u32::try_from(self.credentials.len())
            .expect("a credential table cannot reach u32::MAX entries");
        self.credentials.push(self.token.clone());
        wasmtime::component::Resource::new_own(rep)
    }

    fn request(
        &mut self,
        credential: wasmtime::component::Resource<bindings::Credential>,
        url: String,
    ) -> Result<bindings::Response, String> {
        let token = self
            .credentials
            .get(usize::try_from(credential.rep()).expect("a u32 handle fits in usize"))
            .and_then(Option::as_ref)
            .ok_or_else(|| "the credential handle is spent".to_owned())?;

        // Refuse a host the manifest did not declare, before anything is sent.
        let host = host_of(&url).ok_or_else(|| format!("not a URL: {url}"))?;
        if !self.hosts.iter().any(|allowed| allowed == host) {
            return Err(format!("the manifest does not allow reaching {host}"));
        }

        // Spend the credential on the request, attaching the value the component
        // never holds.
        let mut response = ureq::Agent::new_with_defaults()
            .get(&url)
            .header("Authorization", &format!("Bearer {token}"))
            .call()
            .map_err(|error| error.to_string())?;
        let status = response.status().as_u16();
        let body = response
            .body_mut()
            .read_to_vec()
            .map_err(|error| error.to_string())?;
        Ok(bindings::Response { status, body })
    }
}

impl bindings::HostCredential for Host {
    fn drop(
        &mut self,
        credential: wasmtime::component::Resource<bindings::Credential>,
    ) -> wasmtime::Result<()> {
        if let Some(slot) = self
            .credentials
            .get_mut(usize::try_from(credential.rep()).expect("a u32 handle fits in usize"))
        {
            *slot = None;
        }
        Ok(())
    }
}

/// The authority of a URL, without the scheme, port, path or query: the host a
/// `request` is checked against.
fn host_of(url: &str) -> Option<&str> {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    rest.split(['/', ':', '?', '#'])
        .next()
        .filter(|host| !host.is_empty())
}

/// An address under this provider's name, for failures that have no address of
/// their own to name.
fn synthetic(name: &Provider) -> Address {
    Address::control(
        name.clone(),
        Kind::new("wasm").expect("`wasm` is a valid kind"),
        Name::new("wasm").expect("`wasm` is a valid name"),
    )
}

/// The failure a read reports, naming the first reading when there is one.
fn read_error(name: &Provider, batch: &[Reading], problem: String) -> ProviderError {
    ProviderError::World {
        address: batch
            .first()
            .map_or_else(|| synthetic(name), |reading| reading.address().clone()),
        problem,
    }
}

/// Why a WASM provider could not be loaded.
#[derive(Debug, thiserror::Error)]
pub enum WasmError {
    /// The manifest could not be read from disk.
    #[error("{0}")]
    Io(#[from] std::io::Error),
    /// The manifest is not a manifest.
    #[error("{0}")]
    Manifest(String),
    /// The manifest's name or kinds are not valid address segments.
    #[error("{0}")]
    Schema(String),
    /// The component is not a component wasmtime can load.
    #[error("{0}")]
    Component(String),
}

/// A provider whose name and schema come from a manifest and whose logic lives
/// in a WASM component.
#[derive(Debug)]
pub struct Wasm {
    name: Provider,
    schema: Schema,
    engine: Engine,
    /// The credential the host holds for the component, from the manifest.
    token: Option<String>,
    /// The hosts the component's requests may reach, from the manifest.
    hosts: Vec<String>,
    /// The loaded component. `None` in the fake, which touches nothing.
    component: Option<Component>,
}

impl Wasm {
    /// Loads a provider from its manifest and component.
    ///
    /// The manifest names the provider and declares its kinds; the component is
    /// loaded to prove it is one, and is kept for the operations that arrive in
    /// later work.
    ///
    /// # Errors
    ///
    /// Returns [`WasmError`] when the manifest does not parse, its name or kinds
    /// are not valid address segments, or the component does not load.
    pub fn load(manifest: &Path, component: &Path) -> Result<Self, WasmError> {
        let text = std::fs::read_to_string(manifest)?;
        let manifest = Manifest::read(&text).map_err(WasmError::Manifest)?;
        let engine = Engine::default();
        let component = Component::from_file(&engine, component)
            .map_err(|error| WasmError::Component(error.to_string()))?;
        Ok(Self {
            name: manifest.name().map_err(WasmError::Schema)?,
            schema: manifest.schema().map_err(WasmError::Schema)?,
            engine,
            token: manifest.token().map(str::to_owned),
            hosts: manifest.hosts().to_vec(),
            component: Some(component),
        })
    }

    /// Instantiates the component once, returning a store and its exports.
    ///
    /// # Errors
    ///
    /// Returns the wasmtime or component message when the fake has no component
    /// or the component does not instantiate.
    fn instantiate(&self) -> Result<(Store<Host>, bindings::Provider), String> {
        let Some(component) = &self.component else {
            return Err("the fake WASM provider has no component".to_owned());
        };
        let mut linker = Linker::new(&self.engine);
        bindings::Provider::add_to_linker::<Host, HasSelf<Host>>(&mut linker, |state| state)
            .map_err(|error| error.to_string())?;
        let mut store = Store::new(
            &self.engine,
            Host::new(self.token.clone(), self.hosts.clone()),
        );
        let instance = bindings::Provider::instantiate(&mut store, component, &linker)
            .map_err(|error| error.to_string())?;
        Ok((store, instance))
    }
}

impl ProviderTrait for Wasm {
    fn name(&self) -> &Provider {
        &self.name
    }

    fn schema(&self) -> Schema {
        self.schema.clone()
    }

    fn read(
        &self,
        batch: &[Reading],
        facts: Facts<'_>,
        reach: &dyn Reach,
    ) -> Result<Read, ProviderError> {
        let readings = batch
            .iter()
            .map(convert::reading_to_wit)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|problem| read_error(&self.name, batch, problem))?;
        let facts = convert::facts_to_wit(facts);

        let read = with_reach(reach, || {
            let (mut store, instance) = self
                .instantiate()
                .map_err(|problem| read_error(&self.name, batch, problem))?;
            instance
                .call_read(&mut store, &readings, &facts)
                .map_err(|error| error.to_string())
                .map_err(|problem| read_error(&self.name, batch, problem))?
                .map_err(|problem| read_error(&self.name, batch, problem))
        })?;

        convert::read_result_from_wit(&read)
            .map_err(|problem| read_error(&self.name, batch, problem))
    }

    fn prepare(&self, change: &Prepared<'_>) -> Result<Planned, ProviderError> {
        let prepared =
            convert::prepared_to_wit(change).map_err(|problem| ProviderError::World {
                address: change.address.clone(),
                problem,
            })?;

        let (mut store, instance) = self.instantiate().map_err(|problem| ProviderError::World {
            address: change.address.clone(),
            problem,
        })?;
        let planned = instance
            .call_prepare(&mut store, &prepared)
            .map_err(|error| error.to_string())
            .map_err(|problem| ProviderError::World {
                address: change.address.clone(),
                problem,
            })?
            .map_err(|problem| ProviderError::World {
                address: change.address.clone(),
                problem,
            })?;

        Ok(convert::planned_from_wit(&planned))
    }

    fn conclude(
        &self,
        change: &Concluding<'_>,
        became: &[Answered],
    ) -> Result<Concluded, ProviderError> {
        let concluding =
            convert::concluding_to_wit(change).map_err(|problem| ProviderError::World {
                address: change.address.clone(),
                problem,
            })?;
        let became = became
            .iter()
            .map(convert::answered_to_wit)
            .collect::<Vec<_>>();

        let (mut store, instance) = self.instantiate().map_err(|problem| ProviderError::World {
            address: change.address.clone(),
            problem,
        })?;
        let concluded = instance
            .call_conclude(&mut store, &concluding, &became)
            .map_err(|error| error.to_string())
            .map_err(|problem| ProviderError::World {
                address: change.address.clone(),
                problem,
            })?
            .map_err(|problem| ProviderError::World {
                address: change.address.clone(),
                problem,
            })?;

        convert::concluded_from_wit(&concluded).map_err(|problem| ProviderError::World {
            address: change.address.clone(),
            problem,
        })
    }

    fn act(&self, change: &Prepared<'_>) -> Result<Concluded, ProviderError> {
        Err(ProviderError::World {
            address: change.address.clone(),
            problem: "the WASM provider does not act in the control domain yet".to_owned(),
        })
    }

    fn afterwards(&self, address: &Address, kind: Sending) -> Afterwards {
        let address = convert::address_to_wit(address);
        let kind = convert::sending_to_wit(kind);
        let Ok((mut store, instance)) = self.instantiate() else {
            return Afterwards::Read;
        };
        match instance.call_afterwards(&mut store, &address, kind) {
            Ok(afterwards) => convert::afterwards_from_wit(afterwards),
            Err(_) => Afterwards::Read,
        }
    }

    fn fake() -> Self {
        Self {
            name: Provider::new("wasm").expect("`wasm` is a valid provider name"),
            schema: Schema::new(),
            engine: Engine::default(),
            token: None,
            hosts: Vec::new(),
            component: None,
        }
    }
}
