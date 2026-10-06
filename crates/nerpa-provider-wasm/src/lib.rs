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

mod manifest;

use std::path::Path;

use nerpa_core::Schema;
use nerpa_core::address::{Address, Provider};
use nerpa_core::observed::Facts;
use nerpa_core::provider::{
    Afterwards, Answered, Concluded, Concluding, Planned, Prepared, Provider as ProviderTrait,
    ProviderError, Reach, Read, Reading, Sending,
};
use wasmtime::component::Component;

pub use manifest::Manifest;

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
    /// The loaded component. `None` in the fake, which touches nothing. Kept for
    /// the operations that arrive with the WIT interface in later work.
    #[allow(dead_code, reason = "read by the operations that arrive in later work")]
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
        let engine = wasmtime::Engine::default();
        let component = Component::from_file(&engine, component)
            .map_err(|error| WasmError::Component(error.to_string()))?;
        Ok(Self {
            name: manifest.name().map_err(WasmError::Schema)?,
            schema: manifest.schema().map_err(WasmError::Schema)?,
            component: Some(component),
        })
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
        _facts: Facts<'_>,
        _reach: &dyn Reach,
    ) -> Result<Read, ProviderError> {
        let Some(reading) = batch.first() else {
            return Ok(Read::new());
        };
        Err(ProviderError::World {
            address: reading.address().clone(),
            problem: "the WASM provider loads but does not read yet".to_owned(),
        })
    }

    fn prepare(&self, change: &Prepared<'_>) -> Result<Planned, ProviderError> {
        Err(ProviderError::World {
            address: change.address.clone(),
            problem: "the WASM provider loads but does not plan yet".to_owned(),
        })
    }

    fn conclude(
        &self,
        change: &Concluding<'_>,
        _became: &[Answered],
    ) -> Result<Concluded, ProviderError> {
        Err(ProviderError::World {
            address: change.address.clone(),
            problem: "the WASM provider loads but does not conclude yet".to_owned(),
        })
    }

    fn act(&self, change: &Prepared<'_>) -> Result<Concluded, ProviderError> {
        Err(ProviderError::World {
            address: change.address.clone(),
            problem: "the WASM provider loads but does not act yet".to_owned(),
        })
    }

    fn afterwards(&self, _address: &Address, _kind: Sending) -> Afterwards {
        Afterwards::Read
    }

    fn fake() -> Self {
        Self {
            name: Provider::new("wasm").expect("`wasm` is a valid provider name"),
            schema: Schema::new(),
            component: None,
        }
    }
}
