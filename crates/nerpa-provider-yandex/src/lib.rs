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

use nerpa_core::address::{Address, Provider as ProviderName};
use nerpa_core::observed::Facts;
use nerpa_core::provider::{
    Afterwards, Answered, Attributes, Concluded, Concluding, Planned, Prepared, Provider,
    ProviderError, Reach, Read, Reading, Sending,
};
use nerpa_core::schema::Schema;

use crate::attributes::{
    attributes, bucket_attributes, required_list, required_text, resources, subnet_attributes,
    wire_name,
};
use crate::auth::TokenCache;
use crate::bucket::held_bucket_name;
use crate::fake::Memory;
use crate::wire::{
    CreateInstance, CreateNetwork, CreateSubnet, InstanceList, NetworkList, Pages, PatchInstance,
    SubnetList, WireInstance, WireNetwork, WireSubnet,
};
pub use api::{Api, ApiError, Http, Response};
pub use auth::{Auth, AuthError};
pub use key::{KeyError, ServiceAccountKey};
pub use record::{Exchange, Recorded};
pub use signer::{RingSigner, Signer};

/// Instances, networks and subnets on Yandex Cloud.
pub struct Yandex {
    name: ProviderName,
    folder_id: String,
    compute: String,
    vpc: String,
    pub(crate) storage: String,
    pub(crate) s3: String,
    auth: Auth,
    pub(crate) api: Box<dyn Api>,
    token_cache: TokenCache,
}

impl std::fmt::Debug for Yandex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Yandex")
            .field("name", &self.name)
            .field("folder_id", &self.folder_id)
            .field("compute", &self.compute)
            .field("vpc", &self.vpc)
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
    /// under Storage — and `s3` is the object endpoint a bucket's versioning
    /// is written through (adr-2026-081).
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
            storage: storage.into(),
            s3: s3.into(),
            auth,
            api,
            token_cache: TokenCache::default(),
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

/// An API error, as a refusal about one resource.
pub(crate) fn world<T>(address: &Address, result: Result<T, ApiError>) -> Result<T, ProviderError> {
    result.map_err(|error| ProviderError::World {
        address: address.clone(),
        problem: error.to_string(),
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
            .at_birth(self.name.clone(), instance, "boot_disk_image_id")
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
        Self::new(
            "fake-folder",
            "http://fake-compute",
            "http://fake-vpc",
            "http://fake-storage",
            "http://fake-s3",
            Auth::IamToken("fake".to_owned()),
            Box::new(Memory::default()),
        )
    }
}
