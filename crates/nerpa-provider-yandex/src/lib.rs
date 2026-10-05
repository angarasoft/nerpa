//! Instances on Yandex Cloud, over its REST API.
//!
//! A control-domain provider: there is no machine and no executor, so the
//! provider reaches the API itself inside `read` and `act`. The HTTP client
//! lives behind the crate-local [`Api`] trait, so the engine never sees an HTTP
//! type and the wire is swappable (adr-2026-070).

mod api;
mod auth;

use std::collections::BTreeMap;

use nerpa_core::address::{Address, Provider as ProviderName};
use nerpa_core::observed::Facts;
use nerpa_core::provider::{
    Afterwards, Answered, Attributes, Concluded, Concluding, Planned, Prepared, Provider,
    ProviderError, Reach, Read, Reading, Sending,
};
use nerpa_core::schema::Schema;
use nerpa_core::value::Value;
use serde::{Deserialize, Serialize};

pub use api::{Api, ApiError, Http, Response};
pub use auth::Auth;

/// One gibibyte: Yandex Cloud counts memory in bytes, a person counts in GiB.
const GIB: i64 = 1024 * 1024 * 1024;

/// Instances, networks and subnets on Yandex Cloud.
pub struct Yandex {
    name: ProviderName,
    folder_id: String,
    endpoint: String,
    auth: Auth,
    api: Box<dyn Api>,
}

impl std::fmt::Debug for Yandex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Yandex")
            .field("name", &self.name)
            .field("folder_id", &self.folder_id)
            .field("endpoint", &self.endpoint)
            .field("auth", &self.auth)
            .finish_non_exhaustive()
    }
}

impl Yandex {
    /// A provider for `yandex:instance` in `folder_id`, reached through `api`.
    ///
    /// # Panics
    ///
    /// Panics if `yandex` stops being a valid provider name, which would mean
    /// the address grammar changed underneath this crate.
    #[must_use]
    #[allow(
        clippy::expect_used,
        reason = "a literal that stopped parsing is a bug here"
    )]
    pub fn new(
        folder_id: impl Into<String>,
        endpoint: impl Into<String>,
        auth: Auth,
        api: Box<dyn Api>,
    ) -> Self {
        Self {
            name: ProviderName::new("yandex").expect("`yandex` is a valid provider name"),
            folder_id: folder_id.into(),
            endpoint: endpoint.into(),
            auth,
            api,
        }
    }

    /// The IAM token, resolved for one address.
    fn token(&self, address: &Address) -> Result<String, ProviderError> {
        self.auth
            .resolve(self.api.as_ref())
            .map_err(|error| ProviderError::World {
                address: address.clone(),
                problem: error.to_string(),
            })
    }

    /// The list of instances in the folder.
    fn list(
        &self,
        headers: &[(&str, &str)],
        address: &Address,
    ) -> Result<Vec<WireInstance>, ProviderError> {
        let url = format!("{}/instances?folderId={}", self.endpoint, self.folder_id);
        let response = self
            .api
            .get(&url, headers)
            .map_err(|error| ProviderError::World {
                address: address.clone(),
                problem: error.to_string(),
            })?;
        let list: InstanceList =
            serde_json::from_str(&response.body).map_err(|error| ProviderError::World {
                address: address.clone(),
                problem: error.to_string(),
            })?;
        Ok(list.instances)
    }
}

/// An instance, as the API answers with it.
#[derive(Debug, Deserialize, Serialize)]
struct WireInstance {
    id: String,
    name: String,
    #[serde(rename = "zoneId")]
    zone_id: String,
    #[serde(rename = "platformId")]
    platform_id: String,
    resources: WireResources,
}

/// What an instance is made of.
#[derive(Debug, Deserialize, Serialize)]
struct WireResources {
    cores: i64,
    memory: i64,
}

/// The answer to a list call.
#[derive(Debug, Deserialize)]
struct InstanceList {
    instances: Vec<WireInstance>,
}

/// What a create call is asked with.
#[derive(Debug, Serialize, Deserialize)]
struct CreateInstance {
    #[serde(rename = "folderId")]
    folder_id: String,
    name: String,
    #[serde(rename = "zoneId")]
    zone_id: String,
    #[serde(rename = "platformId")]
    platform_id: String,
    #[serde(rename = "resourcesSpec")]
    resources_spec: WireResources,
}

/// What an update call is asked with: the parts that may change in place.
#[derive(Debug, Serialize, Deserialize)]
struct PatchInstance {
    #[serde(rename = "resourcesSpec")]
    resources_spec: WireResources,
}

/// The attributes a wire instance reads back as.
fn attributes(instance: &WireInstance) -> Attributes {
    let mut attributes = Attributes::new();
    attributes.insert("zone_id".to_owned(), Value::Text(instance.zone_id.clone()));
    attributes.insert(
        "platform_id".to_owned(),
        Value::Text(instance.platform_id.clone()),
    );
    attributes.insert("cores".to_owned(), Value::Integer(instance.resources.cores));
    attributes.insert(
        "memory_gb".to_owned(),
        Value::Integer(instance.resources.memory / GIB),
    );
    attributes
}

/// A required text attribute of a declaration.
fn required_text(
    desired: &Attributes,
    name: &str,
    address: &Address,
) -> Result<String, ProviderError> {
    match desired.get(name) {
        Some(Value::Text(text)) => Ok(text.clone()),
        _ => Err(ProviderError::Declaration {
            address: address.clone(),
            problem: format!("{name} is required and must be text"),
        }),
    }
}

/// A required integer attribute of a declaration.
fn required_integer(
    desired: &Attributes,
    name: &str,
    address: &Address,
) -> Result<i64, ProviderError> {
    match desired.get(name) {
        Some(Value::Integer(value)) => Ok(*value),
        _ => Err(ProviderError::Declaration {
            address: address.clone(),
            problem: format!("{name} is required and must be an integer"),
        }),
    }
}

/// The resources a declaration asks for, in the wire's unit.
fn resources(desired: &Attributes, address: &Address) -> Result<WireResources, ProviderError> {
    Ok(WireResources {
        cores: required_integer(desired, "cores", address)?,
        memory: required_integer(desired, "memory_gb", address)?
            .checked_mul(GIB)
            .ok_or_else(|| ProviderError::Declaration {
                address: address.clone(),
                problem: "memory_gb overflows".to_owned(),
            })?,
    })
}

/// An API error, as a refusal about one resource.
fn world<T>(address: &Address, result: Result<T, ApiError>) -> Result<T, ProviderError> {
    result.map_err(|error| ProviderError::World {
        address: address.clone(),
        problem: error.to_string(),
    })
}

/// The name of a resource, as the wire knows it.
fn wire_name(address: &Address) -> String {
    address.name().as_str().to_owned()
}

impl Provider for Yandex {
    fn name(&self) -> &ProviderName {
        &self.name
    }

    #[allow(
        clippy::expect_used,
        reason = "a literal kind that stopped parsing is a bug here"
    )]
    fn schema(&self) -> Schema {
        let instance = nerpa_core::Kind::new("instance").expect("a literal kind");
        Schema::new()
            .immutable(self.name.clone(), instance.clone(), "zone_id")
            .immutable(self.name.clone(), instance, "platform_id")
    }

    fn read(
        &self,
        batch: &[Reading],
        _facts: Facts<'_>,
        _reach: &dyn Reach,
    ) -> Result<Read, ProviderError> {
        let Some(first) = batch.first() else {
            return Ok(Read::new());
        };
        let address = first.address().clone();
        let token = self.token(&address)?;
        let authorization = format!("Bearer {token}");
        let headers = [("Authorization", authorization.as_str())];
        let list = self.list(&headers, &address)?;
        let mut found = Read::new();
        for reading in batch {
            let name = wire_name(reading.address());
            if let Some(instance) = list.iter().find(|instance| instance.name == name) {
                found = found.with(reading.address().clone(), attributes(instance), Vec::new());
            }
        }
        Ok(found)
    }

    fn prepare(&self, _change: &Prepared<'_>) -> Result<Planned, ProviderError> {
        // A control provider applies through `act`, never through `prepare`; the
        // engine branches before this is reached (p0004).
        Err(ProviderError::NotOnAMachine)
    }

    fn act(&self, change: &Prepared<'_>) -> Result<Concluded, ProviderError> {
        let token = self.token(change.address)?;
        let authorization = format!("Bearer {token}");
        let headers = [("Authorization", authorization.as_str())];
        let name = wire_name(change.address);
        let existing = self
            .list(&headers, change.address)?
            .into_iter()
            .find(|instance| instance.name == name);

        if change.kind == Sending::Delete {
            let Some(instance) = existing else {
                return Ok(Concluded::Gone);
            };
            let url = format!("{}/instances/{}", self.endpoint, instance.id);
            world(change.address, self.api.delete(&url, &headers))?;
            return Ok(Concluded::Gone);
        }

        if let Some(instance) = existing {
            let patch = PatchInstance {
                resources_spec: resources(change.desired, change.address)?,
            };
            let body = serde_json::to_string(&patch).map_err(|error| ProviderError::World {
                address: change.address.clone(),
                problem: error.to_string(),
            })?;
            let url = format!("{}/instances/{}", self.endpoint, instance.id);
            world(change.address, self.api.patch(&url, &headers, &body))?;
        } else {
            let create = CreateInstance {
                folder_id: self.folder_id.clone(),
                name: wire_name(change.address),
                zone_id: required_text(change.desired, "zone_id", change.address)?,
                platform_id: required_text(change.desired, "platform_id", change.address)?,
                resources_spec: resources(change.desired, change.address)?,
            };
            let body = serde_json::to_string(&create).map_err(|error| ProviderError::World {
                address: change.address.clone(),
                problem: error.to_string(),
            })?;
            let url = format!("{}/instances", self.endpoint);
            world(change.address, self.api.post(&url, &headers, &body))?;
        }

        // Read back what the world now holds, rather than echoing the request.
        let name = wire_name(change.address);
        let list = self.list(&headers, change.address)?;
        let instance = list
            .iter()
            .find(|instance| instance.name == name)
            .ok_or_else(|| ProviderError::World {
                address: change.address.clone(),
                problem: "the instance is not there after it was made".to_owned(),
            })?;
        Ok(Concluded::Became(attributes(instance)))
    }

    fn conclude(
        &self,
        _change: &Concluding<'_>,
        _became: &[Answered],
    ) -> Result<Concluded, ProviderError> {
        // A control provider concludes in `act`, not from executor answers.
        Err(ProviderError::NotOnAMachine)
    }

    fn afterwards(&self, _address: &Address, _kind: Sending) -> Afterwards {
        Afterwards::Read
    }

    fn fake() -> Self {
        Self::new(
            "fake-folder",
            "http://fake",
            Auth::IamToken("fake".to_owned()),
            Box::new(Memory::default()),
        )
    }
}

/// An API that keeps instances in memory, for [`Yandex::fake`] and the tests.
#[derive(Debug, Default)]
struct Memory {
    instances: std::sync::Mutex<BTreeMap<String, WireInstance>>,
}

impl Api for Memory {
    fn get(&self, _url: &str, _headers: &[(&str, &str)]) -> Result<Response, ApiError> {
        let instances = self
            .instances
            .lock()
            .map_err(|_| ApiError::Transport("the memory store is poisoned".to_owned()))?;
        let mut listed: Vec<&WireInstance> = instances.values().collect();
        listed.sort_by(|left, right| left.name.cmp(&right.name));
        let body = serde_json::to_string(&serde_json::json!({ "instances": listed }))
            .map_err(|error| ApiError::Transport(error.to_string()))?;
        Ok(Response { status: 200, body })
    }

    fn post(
        &self,
        _url: &str,
        _headers: &[(&str, &str)],
        body: &str,
    ) -> Result<Response, ApiError> {
        let create: CreateInstance =
            serde_json::from_str(body).map_err(|error| ApiError::Transport(error.to_string()))?;
        let mut instances = self
            .instances
            .lock()
            .map_err(|_| ApiError::Transport("the memory store is poisoned".to_owned()))?;
        instances.insert(
            create.name.clone(),
            WireInstance {
                id: format!("id-{}", create.name),
                name: create.name,
                zone_id: create.zone_id,
                platform_id: create.platform_id,
                resources: create.resources_spec,
            },
        );
        Ok(Response {
            status: 200,
            body: "{}".to_owned(),
        })
    }

    fn patch(
        &self,
        url: &str,
        _headers: &[(&str, &str)],
        body: &str,
    ) -> Result<Response, ApiError> {
        let patch: PatchInstance =
            serde_json::from_str(body).map_err(|error| ApiError::Transport(error.to_string()))?;
        let id = url.rsplit('/').next().unwrap_or_default();
        let mut instances = self
            .instances
            .lock()
            .map_err(|_| ApiError::Transport("the memory store is poisoned".to_owned()))?;
        if let Some(instance) = instances.values_mut().find(|instance| instance.id == id) {
            instance.resources = patch.resources_spec;
        }
        Ok(Response {
            status: 200,
            body: "{}".to_owned(),
        })
    }

    fn delete(&self, url: &str, _headers: &[(&str, &str)]) -> Result<Response, ApiError> {
        let id = url.rsplit('/').next().unwrap_or_default();
        let mut instances = self
            .instances
            .lock()
            .map_err(|_| ApiError::Transport("the memory store is poisoned".to_owned()))?;
        instances.retain(|_, instance| instance.id != id);
        Ok(Response {
            status: 200,
            body: "{}".to_owned(),
        })
    }
}
