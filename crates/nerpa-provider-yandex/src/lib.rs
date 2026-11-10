//! Instances, networks, subnets and buckets on Yandex Cloud, over its REST
//! APIs.
//!
//! A control-domain provider: there is no machine and no executor, so the
//! provider reaches the API itself inside `read` and `act`. The HTTP client
//! lives behind the crate-local [`Api`] trait, so the engine never sees an HTTP
//! type and the wire is swappable (adr-2026-070).

mod api;
mod attributes;
mod auth;
mod bucket;
mod fake;
mod key;
mod record;
mod signer;
mod wire;

use std::collections::BTreeMap;
use std::time::Duration;

use nerpa_core::address::{Address, Provider as ProviderName};
use nerpa_core::observed::Facts;
use nerpa_core::provider::{
    Afterwards, Answered, Attributes, Concluded, Concluding, Planned, Prepared, Provider,
    ProviderError, Reach, Read, Reading, Sending,
};
use nerpa_core::schema::Schema;

use crate::attributes::{
    attributes, boot_disk, bucket_attributes, desired_disk_size, interface_specs, required_list,
    required_text, resources, subnet_attributes, wire_name,
};
use crate::auth::TokenCache;
use crate::bucket::held_bucket_name;
use crate::fake::Memory;
use crate::wire::{
    CreateInstance, CreateNetwork, CreateSubnet, InstanceList, NetworkList, Pages, PatchDisk,
    PatchInstance, SubnetList, WireDisk, WireInstance, WireNetwork, WireOperation, WireSubnet,
};
pub use api::{Api, ApiError, Http, Response};
pub use auth::{Auth, AuthError};
pub use key::{KeyError, ServiceAccountKey};
pub use record::{Exchange, Recorded};
pub use signer::{RingSigner, Signer};

/// The most times one operation is asked whether it is done. At two seconds
/// between asks, an instance the cloud has not made in five minutes is the
/// cloud's to say, not a loop to hang a run in.
const MAX_POLLS: usize = 150;

/// Instances, networks and subnets on Yandex Cloud.
pub struct Yandex {
    name: ProviderName,
    folder_id: String,
    compute: String,
    vpc: String,
    operations: String,
    pub(crate) storage: String,
    pub(crate) s3: String,
    auth: Auth,
    pub(crate) api: Box<dyn Api>,
    token_cache: TokenCache,
    /// How long to wait between two asks of one operation. The real cloud
    /// is asked every two seconds; the fake finishes on the second ask, so
    /// it is asked every millisecond.
    poll: Duration,
}

impl std::fmt::Debug for Yandex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Yandex")
            .field("name", &self.name)
            .field("folder_id", &self.folder_id)
            .field("compute", &self.compute)
            .field("vpc", &self.vpc)
            .field("operations", &self.operations)
            .field("storage", &self.storage)
            .field("s3", &self.s3)
            .field("auth", &self.auth)
            .finish_non_exhaustive()
    }
}

impl Yandex {
    /// A provider for `yandex:instance`, `yandex:network`, `yandex:subnet` and
    /// `yandex:bucket` in `folder_id`, reached through `api`.
    ///
    /// `compute`, `vpc` and `storage` are the management endpoints —
    /// instances live under Compute, networks and subnets under VPC, buckets
    /// under Storage — `operations` is where the work a call starts is
    /// asked about until it is done, and `s3` is the object endpoint a
    /// bucket's versioning is written through (adr-2026-081).
    ///
    /// # Panics
    ///
    /// Panics if `yandex` stops being a valid provider name, which would mean
    /// the address grammar changed underneath this crate.
    #[must_use]
    #[allow(
        clippy::too_many_arguments,
        reason = "the endpoints are the doors the wire speaks, named one each rather than folded into a struct"
    )]
    #[allow(
        clippy::expect_used,
        reason = "a literal that stopped parsing is a bug here"
    )]
    pub fn new(
        folder_id: impl Into<String>,
        compute: impl Into<String>,
        vpc: impl Into<String>,
        operations: impl Into<String>,
        storage: impl Into<String>,
        s3: impl Into<String>,
        auth: Auth,
        api: Box<dyn Api>,
    ) -> Self {
        Self {
            name: ProviderName::new("yandex").expect("`yandex` is a valid provider name"),
            folder_id: folder_id.into(),
            compute: compute.into(),
            vpc: vpc.into(),
            operations: operations.into(),
            storage: storage.into(),
            s3: s3.into(),
            auth,
            api,
            token_cache: TokenCache::default(),
            poll: Duration::from_secs(2),
        }
    }

    /// The IAM token, resolved once and reused until it expires.
    fn token(&self, address: &Address) -> Result<String, ProviderError> {
        self.token_cache
            .token(|| self.auth.resolve(self.api.as_ref()))
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

    /// Every page of a list, from the first to the one that names no next:
    /// the API answers in pages, and a read that stopped at the first would
    /// call a folder larger than one page a smaller world than it is.
    fn list_all<L>(
        &self,
        first: &str,
        headers: &[(&str, &str)],
        address: &Address,
    ) -> Result<Vec<L::Item>, ProviderError>
    where
        L: Pages + serde::de::DeserializeOwned,
    {
        let mut all = Vec::new();
        let mut token = None;
        loop {
            let url = match &token {
                Some(page) => format!("{first}&pageToken={page}"),
                None => first.to_owned(),
            };
            let page: L = self.list_of(&url, headers, address)?;
            let (items, next) = page.parts();
            all.extend(items);
            if next.as_deref().is_none_or(str::is_empty) {
                return Ok(all);
            }
            token = next;
        }
    }

    /// The instances in the folder, every page of them.
    fn list_instances(
        &self,
        headers: &[(&str, &str)],
        address: &Address,
    ) -> Result<Vec<WireInstance>, ProviderError> {
        let url = format!("{}/instances?folderId={}", self.compute, self.folder_id);
        self.list_all::<InstanceList>(&url, headers, address)
    }

    /// The networks in the folder, every page of them.
    fn list_networks(
        &self,
        headers: &[(&str, &str)],
        address: &Address,
    ) -> Result<Vec<WireNetwork>, ProviderError> {
        let url = format!("{}/networks?folderId={}", self.vpc, self.folder_id);
        self.list_all::<NetworkList>(&url, headers, address)
    }

    /// The subnets in the folder, every page of them.
    fn list_subnets(
        &self,
        headers: &[(&str, &str)],
        address: &Address,
    ) -> Result<Vec<WireSubnet>, ProviderError> {
        let url = format!("{}/subnets?folderId={}", self.vpc, self.folder_id);
        self.list_all::<SubnetList>(&url, headers, address)
    }

    /// One operation, awaited: asked again and again until the cloud says it
    /// is done, and its failure is the refusal it said. A call the cloud
    /// answers with an operation is a promise, not a fact — the instance a
    /// create promises is not there until the promise is kept, and a stop is
    /// not a stop until it says it is one.
    fn await_operation(
        &self,
        headers: &[(&str, &str)],
        address: &Address,
        answer: &Response,
    ) -> Result<(), ProviderError> {
        let mut operation = operation_of(address, answer)?;
        let mut asks = 0;
        while !operation.done {
            if asks >= MAX_POLLS {
                return Err(ProviderError::World {
                    address: address.clone(),
                    problem: format!("the operation {} never finished", operation.id),
                });
            }
            std::thread::sleep(self.poll);
            let url = format!("{}/{}", self.operations, operation.id);
            let answer = world(address, self.api.get(&url, headers))?;
            operation = operation_of(address, &answer)?;
            asks += 1;
        }
        match operation.error {
            Some(problem) => Err(ProviderError::World {
                address: address.clone(),
                problem: format!(
                    "the operation failed: {}",
                    problem
                        .message
                        .unwrap_or_else(|| "the cloud said nothing".to_owned())
                ),
            }),
            None => Ok(()),
        }
    }

    /// Stops a running instance, and waits until the cloud has stopped it.
    fn stop_instance(
        &self,
        headers: &[(&str, &str)],
        address: &Address,
        instance_id: &str,
    ) -> Result<(), ProviderError> {
        let url = format!("{}/instances/{instance_id}:stop", self.compute);
        let answer = world(address, self.api.post(&url, headers, "{}"))?;
        self.await_operation(headers, address, &answer)
    }

    /// Starts a stopped instance, and waits until the cloud has started it.
    fn start_instance(
        &self,
        headers: &[(&str, &str)],
        address: &Address,
        instance_id: &str,
    ) -> Result<(), ProviderError> {
        let url = format!("{}/instances/{instance_id}:start", self.compute);
        let answer = world(address, self.api.post(&url, headers, "{}"))?;
        self.await_operation(headers, address, &answer)
    }

    /// The disk an instance boots from, where it names one: the instance
    /// answers with the disk's id, and the disk answers with what it holds.
    fn boot_disk_of(
        &self,
        instance: &WireInstance,
        headers: &[(&str, &str)],
        address: &Address,
    ) -> Result<Option<WireDisk>, ProviderError> {
        let Some(boot) = &instance.boot_disk else {
            return Ok(None);
        };
        let url = format!("{}/disks/{}", self.compute, boot.disk_id);
        let answer = world(address, self.api.get(&url, headers))?;
        serde_json::from_str(&answer.body)
            .map_err(|error| ProviderError::World {
                address: address.clone(),
                problem: format!("the answer is not a disk: {error}"),
            })
            .map(Some)
    }

    /// Makes an instance, and waits until the cloud has made it: the create
    /// answers with an operation, and the instance it promises — with its
    /// boot disk and its interface — is not there until the operation is
    /// done.
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
            boot_disk_spec: boot_disk(change.desired, change.address)?,
            network_interface_specs: interface_specs(change.desired, change.address)?,
        };
        let body = serde_json::to_string(&create).map_err(|error| ProviderError::World {
            address: change.address.clone(),
            problem: error.to_string(),
        })?;
        let url = format!("{}/instances", self.compute);
        let answer = world(change.address, self.api.post(&url, headers, &body))?;
        self.await_operation(headers, change.address, &answer)
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
        let disk = self.boot_disk_of(instance, headers, change.address)?;
        Ok(Concluded::Became(attributes(instance, disk.as_ref())))
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
                    let answer = world(change.address, self.api.delete(&url, headers))?;
                    self.await_operation(headers, change.address, &answer)?;
                }
                Ok(Concluded::Gone)
            }
            Sending::Replace => {
                if let Some(instance) = existing {
                    let url = format!("{}/instances/{}", self.compute, instance.id);
                    let answer = world(change.address, self.api.delete(&url, headers))?;
                    self.await_operation(headers, change.address, &answer)?;
                }
                self.create_instance(change, headers)?;
                self.read_instance(change, headers, name)
            }
            Sending::Update => self.update_instance(change, headers, existing, name),
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

    /// Changes an instance where it stands: its resources change only while
    /// it is stopped, and its boot disk only grows. A running instance is
    /// stopped, changed and started again; one already stopped is changed
    /// where it stands and never started, for a stop the declaration did not
    /// ask for is a state it did not ask to leave.
    fn update_instance(
        &self,
        change: &Prepared<'_>,
        headers: &[(&str, &str)],
        existing: Option<WireInstance>,
        name: &str,
    ) -> Result<Concluded, ProviderError> {
        let Some(instance) = existing else {
            return Err(ProviderError::World {
                address: change.address.clone(),
                problem: "the instance to update is not there".to_owned(),
            });
        };
        let desired = resources(change.desired, change.address)?;
        let resources_differ = instance.resources.cores != desired.cores
            || instance.resources.memory != desired.memory;
        let disk = self.boot_disk_of(&instance, headers, change.address)?;
        let grow = match (desired_disk_size(change.desired, change.address)?, &disk) {
            (Some(want), Some(disk)) if disk.size != want => Some((disk.id.clone(), want)),
            (Some(_), None) => {
                return Err(ProviderError::World {
                    address: change.address.clone(),
                    problem: "the instance to grow has no boot disk".to_owned(),
                });
            }
            _ => None,
        };
        if !resources_differ && grow.is_none() {
            return Ok(Concluded::Became(attributes(&instance, disk.as_ref())));
        }
        let running = match instance.status.as_deref() {
            Some("RUNNING") => true,
            Some("STOPPED") => false,
            Some(other) => {
                return Err(ProviderError::World {
                    address: change.address.clone(),
                    problem: format!("an instance in {other} cannot change its resources"),
                });
            }
            None => {
                return Err(ProviderError::World {
                    address: change.address.clone(),
                    problem: "the instance's status is unknown".to_owned(),
                });
            }
        };
        if resources_differ {
            if running {
                self.stop_instance(headers, change.address, &instance.id)?;
            }
            let patch = PatchInstance {
                resources_spec: desired,
            };
            let body = serde_json::to_string(&patch).map_err(|error| ProviderError::World {
                address: change.address.clone(),
                problem: error.to_string(),
            })?;
            let url = format!(
                "{}/instances/{}?updateMask=resourcesSpec",
                self.compute, instance.id
            );
            let answer = world(change.address, self.api.patch(&url, headers, &body))?;
            self.await_operation(headers, change.address, &answer)?;
            if running {
                self.start_instance(headers, change.address, &instance.id)?;
            }
        }
        if let Some((disk_id, size)) = grow {
            let body = serde_json::to_string(&PatchDisk { size }).map_err(|error| {
                ProviderError::World {
                    address: change.address.clone(),
                    problem: error.to_string(),
                }
            })?;
            let url = format!("{}/disks/{disk_id}?updateMask=size", self.compute);
            let answer = world(change.address, self.api.patch(&url, headers, &body))?;
            self.await_operation(headers, change.address, &answer)?;
        }
        self.read_instance(change, headers, name)
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

/// An API error, as a refusal about one resource.
pub(crate) fn world<T>(address: &Address, result: Result<T, ApiError>) -> Result<T, ProviderError> {
    result.map_err(|error| ProviderError::World {
        address: address.clone(),
        problem: error.to_string(),
    })
}

/// The answer a call gave, read as the operation it is.
fn operation_of(address: &Address, answer: &Response) -> Result<WireOperation, ProviderError> {
    serde_json::from_str(&answer.body).map_err(|error| ProviderError::World {
        address: address.clone(),
        problem: format!("the answer is not an operation: {error}"),
    })
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
        let bucket = nerpa_core::Kind::new("bucket").expect("a literal kind");
        Schema::new()
            .creates_node(self.name.clone(), instance.clone())
            .immutable(self.name.clone(), instance.clone(), "zone_id")
            .immutable(self.name.clone(), instance.clone(), "platform_id")
            .immutable(self.name.clone(), instance.clone(), "subnet_id")
            .at_birth(self.name.clone(), instance.clone(), "boot_disk_image_id")
            // A boot disk only grows: a shrink is refused while planning,
            // rather than proposed to a cloud that would refuse it too.
            .monotonic(self.name.clone(), instance.clone(), "boot_disk_gb")
            // The cores and the memory change only while the instance is
            // stopped, so the plan carries the stop the change requires.
            .stop_to_change(self.name.clone(), instance.clone(), "cores")
            .stop_to_change(self.name.clone(), instance, "memory_gb")
            .immutable(self.name.clone(), subnet.clone(), "network_id")
            .immutable(self.name.clone(), subnet.clone(), "zone_id")
            .immutable(self.name.clone(), subnet, "v4_cidr_blocks")
            // The S3 door changes a bucket's versioning in place
            // (adr-2026-081); the management update alone could not.
            .monotonic(self.name.clone(), bucket, "max_size")
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
                            let disk = self.boot_disk_of(instance, &headers, reading.address())?;
                            found = found.with(
                                reading.address().clone(),
                                attributes(instance, disk.as_ref()),
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
                "bucket" => {
                    for reading in readings {
                        let name = held_bucket_name(reading.address())?;
                        if let Some(bucket) = self.get_bucket(&name, &headers, reading.address())? {
                            found = found.with(
                                reading.address().clone(),
                                bucket_attributes(&bucket),
                                Vec::new(),
                            );
                        }
                    }
                }
                _ => {
                    return Err(ProviderError::UnknownKind {
                        provider: self.name.clone(),
                        address: readings
                            .first()
                            .map_or_else(|| address.clone(), |reading| reading.address().clone()),
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
            "bucket" => self.act_bucket(change, &headers),
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
        let mut fake = Self::new(
            "fake-folder",
            "http://fake-compute",
            "http://fake-vpc",
            "http://fake-operations/operations",
            "http://fake-storage",
            "http://fake-s3",
            Auth::IamToken("fake".to_owned()),
            Box::new(Memory::default()),
        );
        // The fake finishes an operation on its second ask: a millisecond
        // between asks keeps a run fast while the asking itself is proven.
        fake.poll = Duration::from_millis(1);
        fake
    }
}

#[cfg(test)]
mod tests {
    // Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
    #![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

    use std::sync::{Arc, Mutex};

    use nerpa_core::address::address;
    use nerpa_core::provider::{Machines, Providers};
    use nerpa_core::run::{apply, observe};
    use nerpa_core::secret::{Key, Remembered, Secrets};
    use nerpa_core::state::State;
    use nerpa_core::{Graph, Resource};

    use super::{Api, ApiError, Auth, Memory, Response, Yandex};

    static DIGEST_KEY: Key = Key::from_bytes([5; 32]);
    static NO_STORE: Remembered = Remembered::EMPTY;

    fn secrets() -> Secrets<'static> {
        Secrets::new(&DIGEST_KEY, &NO_STORE)
    }

    fn plan(
        graph: &Graph,
        observed: &nerpa_core::Observed,
        state: &State,
        schema: &nerpa_core::schema::Schema,
    ) -> nerpa_core::Plan {
        nerpa_core::planner::plan(graph, observed, state, schema, secrets())
            .expect("plannable")
            .attributed_to(
                nerpa_core::Actor::new("tester", nerpa_core::Grant::default(), None)
                    .expect("an actor"),
            )
    }

    /// A wire that keeps every call it was asked, around the fake cloud.
    struct Watching {
        real: Memory,
        calls: Arc<Mutex<Vec<String>>>,
    }

    impl Watching {
        fn note(&self, verb: &str, url: &str, body: Option<&str>) {
            let without_query = url.split('?').next().unwrap_or(url);
            let path = without_query
                .split_once("://")
                .map_or(without_query, |(_, rest)| {
                    rest.find('/').map_or(rest, |at| &rest[at..])
                });
            self.calls
                .lock()
                .expect("the calls are not poisoned")
                .push(format!("{verb} {path} {}", body.unwrap_or("")));
        }
    }

    impl Api for Watching {
        fn get(&self, url: &str, headers: &[(&str, &str)]) -> Result<Response, ApiError> {
            self.note("GET", url, None);
            self.real.get(url, headers)
        }

        fn post(
            &self,
            url: &str,
            headers: &[(&str, &str)],
            body: &str,
        ) -> Result<Response, ApiError> {
            self.note("POST", url, Some(body));
            self.real.post(url, headers, body)
        }

        fn patch(
            &self,
            url: &str,
            headers: &[(&str, &str)],
            body: &str,
        ) -> Result<Response, ApiError> {
            self.note("PATCH", url, Some(body));
            self.real.patch(url, headers, body)
        }

        fn put(
            &self,
            url: &str,
            headers: &[(&str, &str)],
            body: &str,
        ) -> Result<Response, ApiError> {
            self.note("PUT", url, Some(body));
            self.real.put(url, headers, body)
        }

        fn delete(&self, url: &str, headers: &[(&str, &str)]) -> Result<Response, ApiError> {
            self.note("DELETE", url, None);
            self.real.delete(url, headers)
        }
    }

    /// The providers a watched run drives, asking their operations fast.
    fn watched(calls: &Arc<Mutex<Vec<String>>>) -> Providers {
        let mut yandex = Yandex::new(
            "fake-folder",
            "http://fake-compute",
            "http://fake-vpc",
            "http://fake-operations/operations",
            "http://fake-storage",
            "http://fake-s3",
            Auth::IamToken("fake".to_owned()),
            Box::new(Watching {
                real: Memory::default(),
                calls: Arc::clone(calls),
            }),
        );
        yandex.poll = std::time::Duration::from_millis(1);
        Providers::new().with(Box::new(yandex))
    }

    /// The declaration an instance is born from: its zone, its platform, its
    /// resources, the subnet its interface lives in, and the image its boot
    /// disk is made from.
    fn declaration(instance: &nerpa_core::Address, memory_gb: i64, boot_disk_gb: i64) -> Graph {
        let mut graph = Graph::new();
        graph
            .insert(
                Resource::new(instance.clone())
                    .with("zone_id", "ru-central1-b")
                    .with("platform_id", "standard-v3")
                    .with("cores", 2)
                    .with("memory_gb", memory_gb)
                    .with("subnet_id", "id-subnet")
                    .with("boot_disk_image_id", "fd-ubuntu")
                    .with("boot_disk_gb", boot_disk_gb),
            )
            .expect("a resource");
        graph
    }

    /// One declared world, made so.
    fn settles(
        graph: &Graph,
        providers: &Providers,
        machines: &Machines,
        state: &mut State,
    ) -> nerpa_core::Plan {
        let observed = observe(graph, providers, machines, state).expect("observed");
        let planned = plan(graph, &observed, state, &providers.schema());
        apply(
            &planned,
            providers,
            machines,
            &observed,
            state,
            secrets(),
            &mut |_| Ok(()),
        )
        .expect("applied");
        planned
    }

    #[test]
    fn a_running_instance_is_stopped_updated_and_started() {
        let instance = address("yandex:instance.db");
        let calls: Arc<Mutex<Vec<String>>> = Arc::default();
        let providers = watched(&calls);
        let machines = Machines::new();
        let mut state = State::new();

        // The instance is born.
        settles(
            &declaration(&instance, 2, 10),
            &providers,
            &machines,
            &mut state,
        );
        assert!(state.get(&instance).is_some(), "the instance was recorded");

        // Its memory grows: the change reaches it only while it is stopped,
        // and the provider stops it, changes it and starts it again.
        settles(
            &declaration(&instance, 4, 10),
            &providers,
            &machines,
            &mut state,
        );

        let calls = calls.lock().unwrap().join("\n");
        // The create carried a boot disk and an interface, and the operation
        // it was answered with was asked until it was done.
        let create = calls
            .lines()
            .find(|line| line.starts_with("POST /instances {"))
            .expect("the create was asked");
        assert!(
            create.contains("\"bootDiskSpec\""),
            "the create carries a boot disk: {create}"
        );
        assert!(
            create.contains("\"imageId\":\"fd-ubuntu\""),
            "the boot disk is made from the declared image: {create}"
        );
        assert!(
            create.contains("\"networkInterfaceSpecs\""),
            "the create carries network interfaces: {create}"
        );
        assert!(
            create.contains("\"subnetId\":\"id-subnet\""),
            "the interface lives in the declared subnet: {create}"
        );
        assert!(
            calls
                .lines()
                .any(|line| line.starts_with("GET /operations/op-")),
            "the operation is asked until it is done:\n{calls}"
        );
        // The stop came before the change, the change before the start.
        let stop = calls.find(":stop").expect("the instance was stopped");
        let patch = calls
            .find("PATCH /instances/id-db")
            .expect("the resources were changed");
        let start = calls.find(":start").expect("the instance was started");
        assert!(
            stop < patch && patch < start,
            "the stop, the change and the start, in that order:\n{calls}"
        );
    }

    #[test]
    fn a_boot_disk_grows_without_a_stop() {
        let instance = address("yandex:instance.db");
        let calls: Arc<Mutex<Vec<String>>> = Arc::default();
        let providers = watched(&calls);
        let machines = Machines::new();
        let mut state = State::new();

        settles(
            &declaration(&instance, 2, 10),
            &providers,
            &machines,
            &mut state,
        );
        // Only the boot disk grows: a disk grows under a running instance,
        // and stopping it for that would be a stop nothing asked for.
        let graph = declaration(&instance, 2, 11);
        let observed = observe(&graph, &providers, &machines, &state).expect("observed");
        let second = plan(&graph, &observed, &state, &providers.schema());
        apply(
            &second,
            &providers,
            &machines,
            &observed,
            &mut state,
            secrets(),
            &mut |_| Ok(()),
        )
        .expect("applied");

        let calls = calls.lock().unwrap().join("\n");
        assert!(
            calls.contains("PATCH /disks/disk-db"),
            "the boot disk grew:\n{calls}"
        );
        assert!(
            !calls.contains(":stop"),
            "a running instance is not stopped for a disk grow:\n{calls}"
        );

        let observed = observe(&graph, &providers, &machines, &state).expect("observed");
        let third = plan(&graph, &observed, &state, &providers.schema());
        assert_eq!(
            third.changes().count(),
            0,
            "the grown disk is what a run after sees"
        );
    }
}
