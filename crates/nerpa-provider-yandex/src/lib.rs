//! Instances on Yandex Cloud, over its REST API.
//!
//! A control-domain provider: there is no machine and no executor, so the
//! provider reaches the API itself inside `read` and `act`. The HTTP client
//! lives behind the crate-local [`Api`] trait, so the engine never sees an HTTP
//! type and the wire is swappable (adr-2026-070).

mod api;
mod auth;
mod record;

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
pub use record::{Exchange, Recorded};

/// One gibibyte: Yandex Cloud counts memory in bytes, a person counts in GiB.
const GIB: i64 = 1024 * 1024 * 1024;

/// Instances, networks and subnets on Yandex Cloud.
pub struct Yandex {
    name: ProviderName,
    folder_id: String,
    compute: String,
    vpc: String,
    auth: Auth,
    api: Box<dyn Api>,
}

impl std::fmt::Debug for Yandex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Yandex")
            .field("name", &self.name)
            .field("folder_id", &self.folder_id)
            .field("compute", &self.compute)
            .field("vpc", &self.vpc)
            .field("auth", &self.auth)
            .finish_non_exhaustive()
    }
}

impl Yandex {
    /// A provider for `yandex:instance`, `yandex:network` and `yandex:subnet`
    /// in `folder_id`, reached through `api`.
    ///
    /// `compute` and `vpc` are the two service endpoints — instances live under
    /// Compute, networks and subnets under VPC.
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
        compute: impl Into<String>,
        vpc: impl Into<String>,
        auth: Auth,
        api: Box<dyn Api>,
    ) -> Self {
        Self {
            name: ProviderName::new("yandex").expect("`yandex` is a valid provider name"),
            folder_id: folder_id.into(),
            compute: compute.into(),
            vpc: vpc.into(),
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

    /// One list call, read and parsed as `T`.
    fn list_of<T>(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        address: &Address,
    ) -> Result<T, ProviderError>
    where
        T: serde::de::DeserializeOwned,
    {
        let response = self
            .api
            .get(url, headers)
            .map_err(|error| ProviderError::World {
                address: address.clone(),
                problem: error.to_string(),
            })?;
        serde_json::from_str(&response.body).map_err(|error| ProviderError::World {
            address: address.clone(),
            problem: error.to_string(),
        })
    }

    /// The instances in the folder.
    fn list_instances(
        &self,
        headers: &[(&str, &str)],
        address: &Address,
    ) -> Result<Vec<WireInstance>, ProviderError> {
        let url = format!("{}/instances?folderId={}", self.compute, self.folder_id);
        let list: InstanceList = self.list_of(&url, headers, address)?;
        Ok(list.instances)
    }

    /// The networks in the folder.
    fn list_networks(
        &self,
        headers: &[(&str, &str)],
        address: &Address,
    ) -> Result<Vec<WireNetwork>, ProviderError> {
        let url = format!("{}/networks?folderId={}", self.vpc, self.folder_id);
        let list: NetworkList = self.list_of(&url, headers, address)?;
        Ok(list.networks)
    }

    /// The subnets in the folder.
    fn list_subnets(
        &self,
        headers: &[(&str, &str)],
        address: &Address,
    ) -> Result<Vec<WireSubnet>, ProviderError> {
        let url = format!("{}/subnets?folderId={}", self.vpc, self.folder_id);
        let list: SubnetList = self.list_of(&url, headers, address)?;
        Ok(list.subnets)
    }

    /// Makes an instance.
    fn create_instance(
        &self,
        change: &Prepared<'_>,
        headers: &[(&str, &str)],
    ) -> Result<(), ProviderError> {
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
        let url = format!("{}/instances", self.compute);
        world(change.address, self.api.post(&url, headers, &body))?;
        Ok(())
    }

    /// Reads one instance back as what it became.
    fn read_instance(
        &self,
        change: &Prepared<'_>,
        headers: &[(&str, &str)],
        name: &str,
    ) -> Result<Concluded, ProviderError> {
        let list = self.list_instances(headers, change.address)?;
        let instance = list
            .iter()
            .find(|instance| instance.name == name)
            .ok_or_else(|| ProviderError::World {
                address: change.address.clone(),
                problem: "the instance is not there after it was made".to_owned(),
            })?;
        Ok(Concluded::Became(attributes(instance)))
    }

    /// Applies an instance change, and reports what it became.
    fn act_instance(
        &self,
        change: &Prepared<'_>,
        headers: &[(&str, &str)],
        name: &str,
    ) -> Result<Concluded, ProviderError> {
        let existing = self
            .list_instances(headers, change.address)?
            .into_iter()
            .find(|instance| instance.name == name);

        match change.kind {
            Sending::Delete => {
                if let Some(instance) = existing {
                    let url = format!("{}/instances/{}", self.compute, instance.id);
                    world(change.address, self.api.delete(&url, headers))?;
                }
                Ok(Concluded::Gone)
            }
            Sending::Replace => {
                if let Some(instance) = existing {
                    let url = format!("{}/instances/{}", self.compute, instance.id);
                    world(change.address, self.api.delete(&url, headers))?;
                }
                self.create_instance(change, headers)?;
                self.read_instance(change, headers, name)
            }
            Sending::Update => {
                let instance = existing.ok_or_else(|| ProviderError::World {
                    address: change.address.clone(),
                    problem: "the instance to update is not there".to_owned(),
                })?;
                let patch = PatchInstance {
                    resources_spec: resources(change.desired, change.address)?,
                };
                let body = serde_json::to_string(&patch).map_err(|error| ProviderError::World {
                    address: change.address.clone(),
                    problem: error.to_string(),
                })?;
                let url = format!("{}/instances/{}", self.compute, instance.id);
                world(change.address, self.api.patch(&url, headers, &body))?;
                self.read_instance(change, headers, name)
            }
            Sending::Create => {
                self.create_instance(change, headers)?;
                self.read_instance(change, headers, name)
            }
            _ => Err(ProviderError::Declaration {
                address: change.address.clone(),
                problem: "an unknown change kind reached an instance".to_owned(),
            }),
        }
    }

    /// Applies a network change. A network has no state beyond its name, so a
    /// change is either made or removed.
    fn act_network(
        &self,
        change: &Prepared<'_>,
        headers: &[(&str, &str)],
        name: &str,
    ) -> Result<Concluded, ProviderError> {
        let existing = self
            .list_networks(headers, change.address)?
            .into_iter()
            .find(|network| network.name == name);

        if change.kind == Sending::Delete {
            if let Some(network) = existing {
                let url = format!("{}/networks/{}", self.vpc, network.id);
                world(change.address, self.api.delete(&url, headers))?;
            }
            return Ok(Concluded::Gone);
        }

        if existing.is_none() {
            let create = CreateNetwork {
                folder_id: self.folder_id.clone(),
                name: name.to_owned(),
            };
            let body = serde_json::to_string(&create).map_err(|error| ProviderError::World {
                address: change.address.clone(),
                problem: error.to_string(),
            })?;
            let url = format!("{}/networks", self.vpc);
            world(change.address, self.api.post(&url, headers, &body))?;
        }
        // A network reads back as nothing: it is there or it is not.
        Ok(Concluded::Became(Attributes::new()))
    }

    /// Makes a subnet.
    fn create_subnet(
        &self,
        change: &Prepared<'_>,
        headers: &[(&str, &str)],
    ) -> Result<(), ProviderError> {
        let create = CreateSubnet {
            folder_id: self.folder_id.clone(),
            name: wire_name(change.address),
            network_id: required_text(change.desired, "network_id", change.address)?,
            zone_id: required_text(change.desired, "zone_id", change.address)?,
            v4_cidr_blocks: required_list(change.desired, "v4_cidr_blocks", change.address)?,
        };
        let body = serde_json::to_string(&create).map_err(|error| ProviderError::World {
            address: change.address.clone(),
            problem: error.to_string(),
        })?;
        let url = format!("{}/subnets", self.vpc);
        world(change.address, self.api.post(&url, headers, &body))?;
        Ok(())
    }

    /// Reads one subnet back as what it became.
    fn read_subnet(
        &self,
        change: &Prepared<'_>,
        headers: &[(&str, &str)],
        name: &str,
    ) -> Result<Concluded, ProviderError> {
        let list = self.list_subnets(headers, change.address)?;
        let subnet = list
            .iter()
            .find(|subnet| subnet.name == name)
            .ok_or_else(|| ProviderError::World {
                address: change.address.clone(),
                problem: "the subnet is not there after it was made".to_owned(),
            })?;
        Ok(Concluded::Became(subnet_attributes(subnet)))
    }

    /// Applies a subnet change, and reports what it became.
    fn act_subnet(
        &self,
        change: &Prepared<'_>,
        headers: &[(&str, &str)],
        name: &str,
    ) -> Result<Concluded, ProviderError> {
        let existing = self
            .list_subnets(headers, change.address)?
            .into_iter()
            .find(|subnet| subnet.name == name);

        match change.kind {
            Sending::Delete => {
                if let Some(subnet) = existing {
                    let url = format!("{}/subnets/{}", self.vpc, subnet.id);
                    world(change.address, self.api.delete(&url, headers))?;
                }
                Ok(Concluded::Gone)
            }
            Sending::Replace => {
                if let Some(subnet) = existing {
                    let url = format!("{}/subnets/{}", self.vpc, subnet.id);
                    world(change.address, self.api.delete(&url, headers))?;
                }
                self.create_subnet(change, headers)?;
                self.read_subnet(change, headers, name)
            }
            Sending::Create => {
                self.create_subnet(change, headers)?;
                self.read_subnet(change, headers, name)
            }
            Sending::Update => Err(ProviderError::Declaration {
                address: change.address.clone(),
                problem: "a subnet cannot be updated in place".to_owned(),
            }),
            _ => Err(ProviderError::Declaration {
                address: change.address.clone(),
                problem: "an unknown change kind reached a subnet".to_owned(),
            }),
        }
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

/// A VPC network, as the API answers with it. A network has no state beyond its
/// name and its existence.
#[derive(Debug, Deserialize, Serialize)]
struct WireNetwork {
    id: String,
    name: String,
}

/// The answer to a network list call.
#[derive(Debug, Deserialize)]
struct NetworkList {
    networks: Vec<WireNetwork>,
}

/// What a network create call is asked with.
#[derive(Debug, Serialize, Deserialize)]
struct CreateNetwork {
    #[serde(rename = "folderId")]
    folder_id: String,
    name: String,
}

/// A subnet, as the API answers with it.
#[derive(Debug, Deserialize, Serialize)]
struct WireSubnet {
    id: String,
    name: String,
    #[serde(rename = "networkId")]
    network_id: String,
    #[serde(rename = "zoneId")]
    zone_id: String,
    #[serde(rename = "v4CidrBlocks")]
    v4_cidr_blocks: Vec<String>,
}

/// The answer to a subnet list call.
#[derive(Debug, Deserialize)]
struct SubnetList {
    subnets: Vec<WireSubnet>,
}

/// What a subnet create call is asked with.
#[derive(Debug, Serialize, Deserialize)]
struct CreateSubnet {
    #[serde(rename = "folderId")]
    folder_id: String,
    name: String,
    #[serde(rename = "networkId")]
    network_id: String,
    #[serde(rename = "zoneId")]
    zone_id: String,
    #[serde(rename = "v4CidrBlocks")]
    v4_cidr_blocks: Vec<String>,
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

/// The attributes a subnet reads back as.
fn subnet_attributes(subnet: &WireSubnet) -> Attributes {
    let mut attributes = Attributes::new();
    attributes.insert(
        "network_id".to_owned(),
        Value::Text(subnet.network_id.clone()),
    );
    attributes.insert("zone_id".to_owned(), Value::Text(subnet.zone_id.clone()));
    attributes.insert(
        "v4_cidr_blocks".to_owned(),
        Value::List(
            subnet
                .v4_cidr_blocks
                .iter()
                .cloned()
                .map(Value::Text)
                .collect(),
        ),
    );
    attributes
}

/// A required list-of-text attribute of a declaration.
fn required_list(
    desired: &Attributes,
    name: &str,
    address: &Address,
) -> Result<Vec<String>, ProviderError> {
    match desired.get(name) {
        Some(Value::List(values)) => values
            .iter()
            .map(|value| match value {
                Value::Text(text) => Ok(text.clone()),
                _ => Err(ProviderError::Declaration {
                    address: address.clone(),
                    problem: format!("{name} must be a list of text"),
                }),
            })
            .collect(),
        _ => Err(ProviderError::Declaration {
            address: address.clone(),
            problem: format!("{name} is required and must be a list"),
        }),
    }
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
        let subnet = nerpa_core::Kind::new("subnet").expect("a literal kind");
        Schema::new()
            .immutable(self.name.clone(), instance.clone(), "zone_id")
            .immutable(self.name.clone(), instance, "platform_id")
            .immutable(self.name.clone(), subnet.clone(), "network_id")
            .immutable(self.name.clone(), subnet.clone(), "zone_id")
            .immutable(self.name.clone(), subnet, "v4_cidr_blocks")
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

        let mut kinds: BTreeMap<&str, Vec<&Reading>> = BTreeMap::new();
        for reading in batch {
            kinds
                .entry(reading.address().kind().as_str())
                .or_default()
                .push(reading);
        }

        let mut found = Read::new();
        for (kind, readings) in kinds {
            match kind {
                "instance" => {
                    let list = self.list_instances(&headers, &address)?;
                    for reading in readings {
                        if let Some(instance) = list
                            .iter()
                            .find(|instance| instance.name == wire_name(reading.address()))
                        {
                            found = found.with(
                                reading.address().clone(),
                                attributes(instance),
                                Vec::new(),
                            );
                        }
                    }
                }
                "network" => {
                    let list = self.list_networks(&headers, &address)?;
                    for reading in readings {
                        if list
                            .iter()
                            .any(|network| network.name == wire_name(reading.address()))
                        {
                            found = found.with(
                                reading.address().clone(),
                                Attributes::new(),
                                Vec::new(),
                            );
                        }
                    }
                }
                "subnet" => {
                    let list = self.list_subnets(&headers, &address)?;
                    for reading in readings {
                        if let Some(subnet) = list
                            .iter()
                            .find(|subnet| subnet.name == wire_name(reading.address()))
                        {
                            found = found.with(
                                reading.address().clone(),
                                subnet_attributes(subnet),
                                Vec::new(),
                            );
                        }
                    }
                }
                _ => {
                    return Err(ProviderError::UnknownKind {
                        provider: self.name.clone(),
                        address: address.clone(),
                    });
                }
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
        match change.address.kind().as_str() {
            "instance" => self.act_instance(change, &headers, &name),
            "network" => self.act_network(change, &headers, &name),
            "subnet" => self.act_subnet(change, &headers, &name),
            _ => Err(ProviderError::UnknownKind {
                provider: self.name.clone(),
                address: change.address.clone(),
            }),
        }
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
            "http://fake-compute",
            "http://fake-vpc",
            Auth::IamToken("fake".to_owned()),
            Box::new(Memory::default()),
        )
    }
}

/// An API that keeps instances, networks and subnets in memory, for
/// [`Yandex::fake`] and the tests.
#[derive(Debug, Default)]
struct Memory {
    instances: std::sync::Mutex<BTreeMap<String, WireInstance>>,
    networks: std::sync::Mutex<BTreeMap<String, WireNetwork>>,
    subnets: std::sync::Mutex<BTreeMap<String, WireSubnet>>,
}

/// The resource kind a URL addresses: `instances`, `networks` or `subnets`.
fn list_kind(url: &str) -> &str {
    if url.contains("/networks") {
        "networks"
    } else if url.contains("/subnets") {
        "subnets"
    } else {
        "instances"
    }
}

/// A poisoned store, as the transport error it is.
fn poisoned() -> ApiError {
    ApiError::Transport("the memory store is poisoned".to_owned())
}

impl Api for Memory {
    fn get(&self, url: &str, _headers: &[(&str, &str)]) -> Result<Response, ApiError> {
        let body = match list_kind(url) {
            "networks" => {
                let networks = self.networks.lock().map_err(|_| poisoned())?;
                let mut listed: Vec<&WireNetwork> = networks.values().collect();
                listed.sort_by(|left, right| left.name.cmp(&right.name));
                serde_json::to_string(&serde_json::json!({ "networks": listed }))
            }
            "subnets" => {
                let subnets = self.subnets.lock().map_err(|_| poisoned())?;
                let mut listed: Vec<&WireSubnet> = subnets.values().collect();
                listed.sort_by(|left, right| left.name.cmp(&right.name));
                serde_json::to_string(&serde_json::json!({ "subnets": listed }))
            }
            _ => {
                let instances = self.instances.lock().map_err(|_| poisoned())?;
                let mut listed: Vec<&WireInstance> = instances.values().collect();
                listed.sort_by(|left, right| left.name.cmp(&right.name));
                serde_json::to_string(&serde_json::json!({ "instances": listed }))
            }
        }
        .map_err(|error| ApiError::Transport(error.to_string()))?;
        Ok(Response { status: 200, body })
    }

    fn post(&self, url: &str, _headers: &[(&str, &str)], body: &str) -> Result<Response, ApiError> {
        match list_kind(url) {
            "networks" => {
                let create: CreateNetwork = serde_json::from_str(body)
                    .map_err(|error| ApiError::Transport(error.to_string()))?;
                self.networks.lock().map_err(|_| poisoned())?.insert(
                    create.name.clone(),
                    WireNetwork {
                        id: format!("id-{}", create.name),
                        name: create.name,
                    },
                );
            }
            "subnets" => {
                let create: CreateSubnet = serde_json::from_str(body)
                    .map_err(|error| ApiError::Transport(error.to_string()))?;
                self.subnets.lock().map_err(|_| poisoned())?.insert(
                    create.name.clone(),
                    WireSubnet {
                        id: format!("id-{}", create.name),
                        name: create.name,
                        network_id: create.network_id,
                        zone_id: create.zone_id,
                        v4_cidr_blocks: create.v4_cidr_blocks,
                    },
                );
            }
            _ => {
                let create: CreateInstance = serde_json::from_str(body)
                    .map_err(|error| ApiError::Transport(error.to_string()))?;
                self.instances.lock().map_err(|_| poisoned())?.insert(
                    create.name.clone(),
                    WireInstance {
                        id: format!("id-{}", create.name),
                        name: create.name,
                        zone_id: create.zone_id,
                        platform_id: create.platform_id,
                        resources: create.resources_spec,
                    },
                );
            }
        }
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
        // Only instances update in place; networks and subnets replace.
        let patch: PatchInstance =
            serde_json::from_str(body).map_err(|error| ApiError::Transport(error.to_string()))?;
        let id = url.rsplit('/').next().unwrap_or("");
        let mut instances = self.instances.lock().map_err(|_| poisoned())?;
        if let Some(instance) = instances.values_mut().find(|instance| instance.id == id) {
            instance.resources = patch.resources_spec;
        }
        Ok(Response {
            status: 200,
            body: "{}".to_owned(),
        })
    }

    fn delete(&self, url: &str, _headers: &[(&str, &str)]) -> Result<Response, ApiError> {
        let id = url.rsplit('/').next().unwrap_or("");
        match list_kind(url) {
            "networks" => {
                self.networks
                    .lock()
                    .map_err(|_| poisoned())?
                    .retain(|_, network| network.id != id);
            }
            "subnets" => {
                self.subnets
                    .lock()
                    .map_err(|_| poisoned())?
                    .retain(|_, subnet| subnet.id != id);
            }
            _ => {
                self.instances
                    .lock()
                    .map_err(|_| poisoned())?
                    .retain(|_, instance| instance.id != id);
            }
        }
        Ok(Response {
            status: 200,
            body: "{}".to_owned(),
        })
    }
}
