//! Instances, networks, subnets and buckets on Yandex Cloud, over its REST
//! APIs.
//!
//! A control-domain provider: there is no machine and no executor, so the
//! provider reaches the API itself inside `read` and `act`. The HTTP client
//! lives behind the crate-local [`Api`] trait, so the engine never sees an HTTP
//! type and the wire is swappable (adr-2026-070).

mod api;
mod auth;
mod key;
mod record;
mod signer;

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

use crate::auth::TokenCache;
pub use api::{Api, ApiError, Http, Response};
pub use auth::{Auth, AuthError};
pub use key::{KeyError, ServiceAccountKey};
pub use record::{Exchange, Recorded};
pub use signer::{RingSigner, Signer};

/// One gibibyte: Yandex Cloud counts memory in bytes, a person counts in GiB.
const GIB: i64 = 1024 * 1024 * 1024;

/// Instances, networks and subnets on Yandex Cloud.
pub struct Yandex {
    name: ProviderName,
    folder_id: String,
    compute: String,
    vpc: String,
    storage: String,
    s3: String,
    auth: Auth,
    api: Box<dyn Api>,
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

    /// One bucket by name, read in the API's full view: the basic one the
    /// list answers with does not carry the tags. `None` where the bucket is
    /// not there — the API answers 404, which is an answer rather than a
    /// failure.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::World`] where the API could not be asked, or
    /// answered with something other than the bucket or its absence.
    fn get_bucket(
        &self,
        name: &str,
        headers: &[(&str, &str)],
        address: &Address,
    ) -> Result<Option<WireBucket>, ProviderError> {
        let url = format!("{}/buckets/{}?view=VIEW_FULL", self.storage, name);
        match self.api.get(&url, headers) {
            Ok(Response { status: 200, body }) => {
                serde_json::from_str(&body)
                    .map(Some)
                    .map_err(|error| ProviderError::World {
                        address: address.clone(),
                        problem: error.to_string(),
                    })
            }
            Ok(Response { status, .. }) => Err(ProviderError::World {
                address: address.clone(),
                problem: format!("the API answered {status} to a bucket read"),
            }),
            Err(ApiError::Refused { status: 404, .. }) => Ok(None),
            Err(error) => Err(ProviderError::World {
                address: address.clone(),
                problem: error.to_string(),
            }),
        }
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

    /// Makes a bucket.
    fn make_bucket(
        &self,
        change: &Prepared<'_>,
        headers: &[(&str, &str)],
    ) -> Result<(), ProviderError> {
        let create = create_bucket(change.desired, change.address, &self.folder_id)?;
        let body = serde_json::to_string(&create).map_err(|error| ProviderError::World {
            address: change.address.clone(),
            problem: error.to_string(),
        })?;
        let url = format!("{}/buckets", self.storage);
        world(change.address, self.api.post(&url, headers, &body))?;
        Ok(())
    }

    /// Reads one bucket back as what it became.
    fn read_bucket(
        &self,
        change: &Prepared<'_>,
        headers: &[(&str, &str)],
        name: &str,
    ) -> Result<Concluded, ProviderError> {
        let bucket = self
            .get_bucket(name, headers, change.address)?
            .ok_or_else(|| ProviderError::World {
                address: change.address.clone(),
                problem: "the bucket is not there after it was made".to_owned(),
            })?;
        Ok(Concluded::Became(bucket_attributes(&bucket)))
    }

    /// Removes a bucket by name. The API answers 404 where there is nothing
    /// to remove, which is what the removal wanted anyway.
    fn remove_bucket(
        &self,
        change: &Prepared<'_>,
        headers: &[(&str, &str)],
        name: &str,
    ) -> Result<(), ProviderError> {
        let url = format!("{}/buckets/{}", self.storage, name);
        match self.api.delete(&url, headers) {
            // The API answers 404 where there is nothing to remove, which is
            // what the removal wanted anyway.
            Ok(_) | Err(ApiError::Refused { status: 404, .. }) => Ok(()),
            Err(error) => Err(ProviderError::World {
                address: change.address.clone(),
                problem: error.to_string(),
            }),
        }
    }

    /// Corrects a bucket in place. The management update cannot carry a
    /// versioning but the enabled word its validation demands, so the
    /// attributes it manages are corrected first with that word in the body,
    /// and the S3 door then writes the versioning the declaration asks for —
    /// the final word (adr-2026-081). A declaration that does not manage
    /// versioning, on a bucket whose versioning is not enabled, is refused by
    /// name: the enabled word the management body carries would flip it.
    fn correct_bucket(
        &self,
        change: &Prepared<'_>,
        headers: &[(&str, &str)],
        name: &str,
    ) -> Result<(), ProviderError> {
        let desired = change.desired;
        let manages = ["default_storage_class", "max_size", "tags"]
            .iter()
            .any(|attribute| desired.get(*attribute).is_some());
        if !manages {
            if desired.get("versioning").is_some() {
                return self.write_versioning(change, headers, name);
            }
            return Err(ProviderError::Declaration {
                address: change.address.clone(),
                problem: "an update with nothing the storage API can change".to_owned(),
            });
        }

        let current = self
            .get_bucket(name, headers, change.address)?
            .ok_or_else(|| ProviderError::World {
                address: change.address.clone(),
                problem: "the bucket to update is not there".to_owned(),
            })?;
        let held = current
            .versioning
            .clone()
            .unwrap_or_else(|| "VERSIONING_DISABLED".to_owned());
        if held != "VERSIONING_ENABLED" && desired.get("versioning").is_none() {
            return Err(ProviderError::World {
                address: change.address.clone(),
                problem: format!(
                    "the management update carries the enabled word, which \
                     would flip this bucket's versioning ({held}); declare \
                     versioning to change it, or enable it first"
                ),
            });
        }
        let patch = PatchBucket {
            versioning: "VERSIONING_ENABLED".to_owned(),
            default_storage_class: optional_text(desired, "default_storage_class", change.address)?
                .or_else(|| current.default_storage_class.clone()),
            max_size: optional_integer(desired, "max_size", change.address)?.or(current.max_size),
            tags: match desired.get("tags") {
                Some(_) => wire_tags(desired, change.address)?,
                None => current.tags.clone(),
            },
            // Carried from the read, where the bucket holds them: the wire
            // applies what the body carries, mask or no mask, so the fields
            // the declaration does not name must say what the world already
            // says. Where the read carried none, the shapes the validation
            // demands are stated: the check off with the console forced,
            // the lock disabled.
            allowed_private_endpoints: current.allowed_private_endpoints.clone().unwrap_or(
                WireAllowedPrivateEndpoints {
                    enabled: false,
                    force_cloud_console_access: true,
                },
            ),
            object_lock: current.object_lock.clone().unwrap_or(WireObjectLock {
                status: "OBJECT_LOCK_STATUS_DISABLED".to_owned(),
            }),
        };
        let mut mask = Vec::new();
        if desired.get("default_storage_class").is_some() {
            mask.push("defaultStorageClass");
        }
        if desired.get("max_size").is_some() {
            mask.push("maxSize");
        }
        if desired.get("tags").is_some() {
            mask.push("tags");
        }
        let body = serde_json::to_string(&patch).map_err(|error| ProviderError::World {
            address: change.address.clone(),
            problem: error.to_string(),
        })?;
        let url = format!(
            "{}/buckets/{}?updateMask={}",
            self.storage,
            name,
            mask.join(",")
        );
        world(change.address, self.api.patch(&url, headers, &body))?;
        if desired.get("versioning").is_some() {
            return self.write_versioning(change, headers, name);
        }
        Ok(())
    }

    /// Writes a bucket's versioning through the S3 door: the one document
    /// that door speaks, with the word the declaration asks for
    /// (adr-2026-081). The token the door takes is the same short-lived one
    /// the management door takes, as a bearer and with no signature.
    fn write_versioning(
        &self,
        change: &Prepared<'_>,
        headers: &[(&str, &str)],
        name: &str,
    ) -> Result<(), ProviderError> {
        let Some(declared) = optional_versioning(change.desired, change.address)? else {
            return Err(ProviderError::Declaration {
                address: change.address.clone(),
                problem: "a versioning update with no versioning declared".to_owned(),
            });
        };
        let status = match declared {
            "enabled" => "Enabled",
            _ => "Suspended",
        };
        let body =
            format!("<VersioningConfiguration><Status>{status}</Status></VersioningConfiguration>");
        let url = format!("{}/{name}?versioning", self.s3);
        let mut xml = headers.to_vec();
        xml.push(("Content-Type", "application/xml"));
        world(change.address, self.api.put(&url, &xml, &body)).map(|_| ())
    }

    /// Applies a bucket change, and reports what it became. The storage API
    /// names a bucket by its name, which is also its id.
    fn act_bucket(
        &self,
        change: &Prepared<'_>,
        headers: &[(&str, &str)],
        name: &str,
    ) -> Result<Concluded, ProviderError> {
        match change.kind {
            Sending::Delete => {
                self.remove_bucket(change, headers, name)?;
                Ok(Concluded::Gone)
            }
            Sending::Replace => {
                self.remove_bucket(change, headers, name)?;
                self.make_bucket(change, headers)?;
                self.read_bucket(change, headers, name)
            }
            Sending::Update => {
                self.correct_bucket(change, headers, name)?;
                self.read_bucket(change, headers, name)
            }
            Sending::Create => {
                self.make_bucket(change, headers)?;
                self.read_bucket(change, headers, name)
            }
            _ => Err(ProviderError::Declaration {
                address: change.address.clone(),
                problem: "an unknown change kind reached a bucket".to_owned(),
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
    #[serde(rename = "networkInterfaces", default)]
    network_interfaces: Vec<WireInterface>,
}

/// One network interface of an instance.
#[derive(Debug, Deserialize, Serialize)]
struct WireInterface {
    #[serde(rename = "subnetId")]
    subnet_id: String,
}

/// What an instance is made of.
#[derive(Debug, Deserialize, Serialize)]
struct WireResources {
    #[serde(with = "int64")]
    cores: i64,
    #[serde(with = "int64")]
    memory: i64,
}

/// A Yandex Cloud int64 crosses JSON as text, because a JSON number would not
/// survive the value's precision. A hand-written fixture may still write a
/// number, so both are read; it is always written as text.
mod int64 {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<i64, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum AsInt64 {
            Text(String),
            Number(i64),
        }
        match AsInt64::deserialize(deserializer)? {
            AsInt64::Text(text) => text.parse().map_err(serde::de::Error::custom),
            AsInt64::Number(number) => Ok(number),
        }
    }

    #[allow(
        clippy::trivially_copy_pass_by_ref,
        reason = "serde's `with` calls the serializer with a reference"
    )]
    pub(crate) fn serialize<S>(value: &i64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value.to_string().serialize(serializer)
    }
}

/// An int64 that may not be there, read the way [`int64`] reads one that is:
/// as text, or as a number a hand-written fixture wrote.
mod optional_int64 {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum AsInt64 {
            Text(String),
            Number(i64),
        }
        match Option::<AsInt64>::deserialize(deserializer)? {
            None => Ok(None),
            Some(AsInt64::Text(text)) => text
                .parse::<i64>()
                .map(Some)
                .map_err(serde::de::Error::custom),
            Some(AsInt64::Number(number)) => Ok(Some(number)),
        }
    }

    #[allow(
        clippy::ref_option,
        reason = "serde's `with` calls the serializer with a reference"
    )]
    pub(crate) fn serialize<S>(value: &Option<i64>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            Some(number) => number.to_string().serialize(serializer),
            None => serializer.serialize_none(),
        }
    }
}

/// The answer to an instance list call. The field is absent where the folder
/// holds no instances: proto3 JSON omits a list that is empty, so an empty
/// folder answers `{}`.
#[derive(Debug, Deserialize)]
struct InstanceList {
    #[serde(default)]
    instances: Vec<WireInstance>,
    /// The page after this one, where the folder holds more than a page.
    #[serde(rename = "nextPageToken", default)]
    next_page_token: Option<String>,
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

/// The answer to a network list call. The field is absent where the folder
/// holds no networks: proto3 JSON omits a list that is empty, so an empty
/// folder answers `{}`.
#[derive(Debug, Deserialize)]
struct NetworkList {
    #[serde(default)]
    networks: Vec<WireNetwork>,
    /// The page after this one, where the folder holds more than a page.
    #[serde(rename = "nextPageToken", default)]
    next_page_token: Option<String>,
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

/// The answer to a subnet list call. The field is absent where the folder
/// holds no subnets: proto3 JSON omits a list that is empty, so an empty
/// folder answers `{}`.
#[derive(Debug, Deserialize)]
struct SubnetList {
    #[serde(default)]
    subnets: Vec<WireSubnet>,
    /// The page after this one, where the folder holds more than a page.
    #[serde(rename = "nextPageToken", default)]
    next_page_token: Option<String>,
}

/// One page of a list, and what names the next: the API answers its lists in
/// pages, and a folder larger than one page is a folder like any other —
/// everything in it is part of the world the read answers for.
trait Pages {
    /// The resource this page holds.
    type Item;
    /// The page's resources, and the token that names the page after it.
    fn parts(self) -> (Vec<Self::Item>, Option<String>);
}

impl Pages for InstanceList {
    type Item = WireInstance;
    fn parts(self) -> (Vec<WireInstance>, Option<String>) {
        (self.instances, self.next_page_token)
    }
}

impl Pages for NetworkList {
    type Item = WireNetwork;
    fn parts(self) -> (Vec<WireNetwork>, Option<String>) {
        (self.networks, self.next_page_token)
    }
}

impl Pages for SubnetList {
    type Item = WireSubnet;
    fn parts(self) -> (Vec<WireSubnet>, Option<String>) {
        (self.subnets, self.next_page_token)
    }
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

/// A bucket, as the API answers with it. A bucket is named by its name, which
/// is unique across the whole platform and is also its id.
#[derive(Debug, Deserialize, Serialize)]
struct WireBucket {
    name: String,
    #[serde(
        rename = "defaultStorageClass",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    default_storage_class: Option<String>,
    /// `VERSIONING_ENABLED`, `VERSIONING_SUSPENDED` or `VERSIONING_DISABLED`.
    /// The API omits a disabled one, because it is the zero value.
    #[serde(
        rename = "versioning",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    versioning: Option<String>,
    #[serde(
        rename = "maxSize",
        with = "optional_int64",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    max_size: Option<i64>,
    #[serde(rename = "tags", default, skip_serializing_if = "Vec::is_empty")]
    tags: Vec<WireTag>,
    /// The object lock the bucket holds, where the full view carries one
    /// back: a WORM setting an update must carry as it found it, not reset.
    #[serde(
        rename = "objectLock",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    object_lock: Option<WireObjectLock>,
    /// The private-endpoint policy the bucket holds, where it carries one.
    #[serde(
        rename = "allowedPrivateEndpoints",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    allowed_private_endpoints: Option<WireAllowedPrivateEndpoints>,
}

/// One tag on a bucket, as the API answers with it.
#[derive(Debug, Clone, Deserialize, Serialize)]
struct WireTag {
    key: String,
    value: String,
}

/// What a bucket create call is asked with. Only what the declaration manages
/// is carried: a field the declaration does not name is left to the cloud's
/// own default.
#[derive(Debug, Serialize, Deserialize)]
struct CreateBucket {
    #[serde(rename = "folderId")]
    folder_id: String,
    name: String,
    #[serde(
        rename = "defaultStorageClass",
        skip_serializing_if = "Option::is_none"
    )]
    default_storage_class: Option<String>,
    #[serde(rename = "versioning", skip_serializing_if = "Option::is_none")]
    versioning: Option<String>,
    #[serde(
        rename = "maxSize",
        with = "optional_int64",
        skip_serializing_if = "Option::is_none"
    )]
    max_size: Option<i64>,
    #[serde(rename = "tags", default, skip_serializing_if = "Vec::is_empty")]
    tags: Vec<WireTag>,
}

/// What a bucket update call is asked with. The update mask travels in the
/// URL; the storage API applies what the body carries whether the mask names
/// it or not, so the body says every field it wants left alone as well as
/// every field it wants changed. Versioning is always `VERSIONING_ENABLED`,
/// the one value the update accepts — the schema declares the attribute
/// immutable, so a change replaces the bucket rather than passing through
/// here. The private-endpoint policy and the object lock are the shapes the
/// update validation demands be present; they say the check off and the lock
/// disabled, and forcing the console's access is what makes the policy count
/// as stated.
#[derive(Debug, Serialize, Deserialize)]
struct PatchBucket {
    #[serde(rename = "versioning")]
    versioning: String,
    #[serde(
        rename = "defaultStorageClass",
        skip_serializing_if = "Option::is_none"
    )]
    default_storage_class: Option<String>,
    #[serde(
        rename = "maxSize",
        with = "optional_int64",
        skip_serializing_if = "Option::is_none"
    )]
    max_size: Option<i64>,
    #[serde(rename = "tags", default, skip_serializing_if = "Vec::is_empty")]
    tags: Vec<WireTag>,
    #[serde(rename = "allowedPrivateEndpoints")]
    allowed_private_endpoints: WireAllowedPrivateEndpoints,
    #[serde(rename = "objectLock")]
    object_lock: WireObjectLock,
}

/// The private-endpoint policy a bucket holds and an update carries: the
/// check, and whether the console's access is forced.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct WireAllowedPrivateEndpoints {
    enabled: bool,
    #[serde(rename = "forceCloudConsoleAccess")]
    force_cloud_console_access: bool,
}

/// The object lock a bucket holds and an update carries.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct WireObjectLock {
    status: String,
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
    if let Some(interface) = instance.network_interfaces.first() {
        attributes.insert(
            "subnet_id".to_owned(),
            Value::Text(interface.subnet_id.clone()),
        );
    }
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

/// The attributes a wire bucket reads back as. Versioning is always reported,
/// and a bucket with none is reported as disabled: the API omits the zero
/// value, and a declaration of `"disabled"` would never converge against an
/// absence. Tags are always reported as a map, empty where there are none, for
/// the same reason: a declared empty map clears them, and a cleared bucket
/// must read back as cleared. An attribute the declaration does not name is
/// reported and never turned into work.
fn bucket_attributes(bucket: &WireBucket) -> Attributes {
    let mut attributes = Attributes::new();
    if let Some(class) = &bucket.default_storage_class {
        attributes.insert(
            "default_storage_class".to_owned(),
            Value::Text(class.clone()),
        );
    }
    let versioning = bucket.versioning.as_deref().map_or_else(
        || "disabled".to_owned(),
        |wire| declared_versioning(wire).map_or_else(|| wire.to_owned(), str::to_owned),
    );
    attributes.insert("versioning".to_owned(), Value::Text(versioning));
    if let Some(size) = bucket.max_size {
        attributes.insert("max_size".to_owned(), Value::Integer(size));
    }
    attributes.insert("tags".to_owned(), Value::Map(tag_map(&bucket.tags)));
    attributes
}

/// The tags a wire bucket holds, as a value map.
fn tag_map(tags: &[WireTag]) -> BTreeMap<String, Value> {
    tags.iter()
        .map(|tag| (tag.key.clone(), Value::Text(tag.value.clone())))
        .collect()
}

/// A declared versioning word, in the management API's spelling.
fn wire_versioning(declared: &str) -> &'static str {
    match declared {
        "enabled" => "VERSIONING_ENABLED",
        _ => "VERSIONING_SUSPENDED",
    }
}

/// The API's versioning word, as a declaration spells it.
fn declared_versioning(wire: &str) -> Option<&'static str> {
    match wire {
        "VERSIONING_ENABLED" => Some("enabled"),
        "VERSIONING_SUSPENDED" => Some("suspended"),
        "VERSIONING_DISABLED" => Some("disabled"),
        _ => None,
    }
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

/// A text attribute a declaration may manage, or `None` where it does not.
fn optional_text(
    desired: &Attributes,
    name: &str,
    address: &Address,
) -> Result<Option<String>, ProviderError> {
    match desired.get(name) {
        None => Ok(None),
        Some(Value::Text(text)) => Ok(Some(text.clone())),
        Some(_) => Err(ProviderError::Declaration {
            address: address.clone(),
            problem: format!("{name} must be text where it is declared"),
        }),
    }
}

/// An integer attribute a declaration may manage, or `None` where it does not.
fn optional_integer(
    desired: &Attributes,
    name: &str,
    address: &Address,
) -> Result<Option<i64>, ProviderError> {
    match desired.get(name) {
        None => Ok(None),
        Some(Value::Integer(value)) => Ok(Some(*value)),
        Some(_) => Err(ProviderError::Declaration {
            address: address.clone(),
            problem: format!("{name} must be an integer where it is declared"),
        }),
    }
}

/// The versioning a declaration manages, as the word it declared, or `None`
/// where the declaration leaves it alone. The words are the two the doors can
/// write: a bucket that was never versioned reads as disabled and stays so
/// until it is declared, because no door can unversion one (adr-2026-081).
fn optional_versioning<'a>(
    desired: &'a Attributes,
    address: &Address,
) -> Result<Option<&'a str>, ProviderError> {
    match desired.get("versioning") {
        None => Ok(None),
        Some(Value::Text(text)) => match text.as_str() {
            "enabled" | "suspended" => Ok(Some(text.as_str())),
            word => Err(ProviderError::Declaration {
                address: address.clone(),
                problem: format!(
                    "versioning is {word}; it is one of enabled and suspended, \
                     and a bucket that was never versioned reads as disabled \
                     until it is declared"
                ),
            }),
        },
        Some(_) => Err(ProviderError::Declaration {
            address: address.clone(),
            problem: "versioning must be text where it is declared".to_owned(),
        }),
    }
}

/// The tags a declaration manages, in the API's spelling, or an empty list
/// where the declaration leaves them alone. An empty declared map clears the
/// tags, so it is carried as an empty list rather than as an absence.
fn wire_tags(desired: &Attributes, address: &Address) -> Result<Vec<WireTag>, ProviderError> {
    match desired.get("tags") {
        None => Ok(Vec::new()),
        Some(Value::Map(entries)) => entries
            .iter()
            .map(|(key, value)| match value {
                Value::Text(text) => Ok(WireTag {
                    key: key.clone(),
                    value: text.clone(),
                }),
                _ => Err(ProviderError::Declaration {
                    address: address.clone(),
                    problem: format!("tags hold text; {key} does not"),
                }),
            })
            .collect(),
        Some(_) => Err(ProviderError::Declaration {
            address: address.clone(),
            problem: "tags must be a map of text where they are declared".to_owned(),
        }),
    }
}

/// The create request a declared bucket becomes. Only what the declaration
/// manages is carried.
fn create_bucket(
    desired: &Attributes,
    address: &Address,
    folder_id: &str,
) -> Result<CreateBucket, ProviderError> {
    Ok(CreateBucket {
        folder_id: folder_id.to_owned(),
        name: wire_name(address),
        default_storage_class: optional_text(desired, "default_storage_class", address)?,
        versioning: optional_versioning(desired, address)?
            .map(wire_versioning)
            .map(str::to_owned),
        max_size: optional_integer(desired, "max_size", address)?,
        tags: wire_tags(desired, address)?,
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
                        let name = wire_name(reading.address());
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
            "bucket" => self.act_bucket(change, &headers, &name),
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

/// An API that keeps instances, networks, subnets and buckets in memory, for
/// [`Yandex::fake`] and the tests.
#[derive(Debug, Default)]
struct Memory {
    instances: std::sync::Mutex<BTreeMap<String, WireInstance>>,
    networks: std::sync::Mutex<BTreeMap<String, WireNetwork>>,
    subnets: std::sync::Mutex<BTreeMap<String, WireSubnet>>,
    buckets: std::sync::Mutex<BTreeMap<String, WireBucket>>,
}

/// The name a single-bucket URL addresses, where it addresses one.
fn bucket_name(url: &str) -> Option<&str> {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    path.rsplit_once("/buckets/").map(|(_, name)| name)
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

/// What the API answers where a bucket is not there.
fn not_there() -> String {
    r#"{"code":5,"message":"The specified bucket does not exist."}"#.to_owned()
}

/// A poisoned store, as the transport error it is.
fn poisoned() -> ApiError {
    ApiError::Transport("the memory store is poisoned".to_owned())
}

impl Api for Memory {
    fn get(&self, url: &str, _headers: &[(&str, &str)]) -> Result<Response, ApiError> {
        // One bucket by name, in the full view the provider asks for, with
        // the 404 the real API answers where it is not there.
        if let Some(name) = bucket_name(url) {
            let buckets = self.buckets.lock().map_err(|_| poisoned())?;
            return match buckets.get(name) {
                Some(bucket) => {
                    let body = serde_json::to_string(bucket)
                        .map_err(|error| ApiError::Transport(error.to_string()))?;
                    Ok(Response { status: 200, body })
                }
                None => Response {
                    status: 404,
                    body: not_there(),
                }
                .or_refused(),
            };
        }
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
        if url.contains("/buckets") {
            let create: CreateBucket = serde_json::from_str(body)
                .map_err(|error| ApiError::Transport(error.to_string()))?;
            self.buckets.lock().map_err(|_| poisoned())?.insert(
                create.name.clone(),
                WireBucket {
                    name: create.name,
                    default_storage_class: create.default_storage_class,
                    versioning: create.versioning,
                    max_size: create.max_size,
                    tags: create.tags,
                    object_lock: None,
                    allowed_private_endpoints: None,
                },
            );
            return Ok(Response {
                status: 200,
                body: "{}".to_owned(),
            });
        }
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
                        network_interfaces: Vec::new(),
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
        // A bucket is corrected by what the body carries — the mask is
        // decorative on the real wire — and an instance through its
        // resources. Networks and subnets replace rather than patch.
        if let Some(name) = bucket_name(url) {
            let patch: PatchBucket = serde_json::from_str(body)
                .map_err(|error| ApiError::Transport(error.to_string()))?;
            let mut buckets = self.buckets.lock().map_err(|_| poisoned())?;
            if let Some(bucket) = buckets.get_mut(name) {
                bucket.default_storage_class = patch.default_storage_class;
                bucket.versioning = Some(patch.versioning);
                bucket.max_size = patch.max_size;
                bucket.tags = patch.tags;
            }
            return Ok(Response {
                status: 200,
                body: "{}".to_owned(),
            });
        }
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

    fn put(&self, url: &str, _headers: &[(&str, &str)], body: &str) -> Result<Response, ApiError> {
        // The S3 door writes the versioning document: the one sub-resource
        // this fake answers for, the one the real door speaks.
        if !url.contains("?versioning") {
            return Err(ApiError::Transport(format!(
                "this fake does not speak PUT for {url}"
            )));
        }
        let path = url.split('?').next().unwrap_or("");
        let name = path.rsplit('/').next().unwrap_or("");
        let enabled = body.contains("<Status>Enabled</Status>");
        let suspended = body.contains("<Status>Suspended</Status>");
        if !enabled && !suspended {
            return Err(ApiError::Transport(format!(
                "this fake does not read the versioning document: {body}"
            )));
        }
        let mut buckets = self.buckets.lock().map_err(|_| poisoned())?;
        if let Some(bucket) = buckets.get_mut(name) {
            bucket.versioning = Some(if enabled {
                "VERSIONING_ENABLED".to_owned()
            } else {
                "VERSIONING_SUSPENDED".to_owned()
            });
        }
        Ok(Response {
            status: 200,
            body: String::new(),
        })
    }

    fn delete(&self, url: &str, _headers: &[(&str, &str)]) -> Result<Response, ApiError> {
        // A bucket is removed by name, with the 404 the real API answers
        // where there was nothing to remove.
        if let Some(name) = bucket_name(url) {
            let removed = self
                .buckets
                .lock()
                .map_err(|_| poisoned())?
                .remove(name)
                .is_some();
            return if removed {
                Ok(Response {
                    status: 200,
                    body: "{}".to_owned(),
                })
            } else {
                Response {
                    status: 404,
                    body: not_there(),
                }
                .or_refused()
            };
        }
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
