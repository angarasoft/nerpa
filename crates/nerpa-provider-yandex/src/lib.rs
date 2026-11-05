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
            .field("auth", &self.auth)
            .finish_non_exhaustive()
    }
}

impl Yandex {
    /// A provider for `yandex:instance`, `yandex:network`, `yandex:subnet` and
    /// `yandex:bucket` in `folder_id`, reached through `api`.
    ///
    /// `compute`, `vpc` and `storage` are the three service endpoints —
    /// instances live under Compute, networks and subnets under VPC, buckets
    /// under Storage.
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
        auth: Auth,
        api: Box<dyn Api>,
    ) -> Self {
        Self {
            name: ProviderName::new("yandex").expect("`yandex` is a valid provider name"),
            folder_id: folder_id.into(),
            compute: compute.into(),
            vpc: vpc.into(),
            storage: storage.into(),
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

    /// The buckets in the folder. A bucket's name is unique across the whole
    /// platform, so a name says which bucket it is without an id.
    fn list_buckets(
        &self,
        headers: &[(&str, &str)],
        address: &Address,
    ) -> Result<Vec<WireBucket>, ProviderError> {
        let url = format!("{}/buckets?folderId={}", self.storage, self.folder_id);
        let list: BucketList = self.list_of(&url, headers, address)?;
        Ok(list.buckets)
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
    fn create_bucket(
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
        let list = self.list_buckets(headers, change.address)?;
        let bucket = list
            .iter()
            .find(|bucket| bucket.name == name)
            .ok_or_else(|| ProviderError::World {
                address: change.address.clone(),
                problem: "the bucket is not there after it was made".to_owned(),
            })?;
        Ok(Concluded::Became(bucket_attributes(bucket)))
    }

    /// Corrects a bucket in place. The storage API changes what the update
    /// mask names, so the mask is exactly what the declaration manages.
    fn update_bucket(
        &self,
        change: &Prepared<'_>,
        headers: &[(&str, &str)],
        name: &str,
    ) -> Result<(), ProviderError> {
        let patch = PatchBucket {
            default_storage_class: optional_text(
                change.desired,
                "default_storage_class",
                change.address,
            )?,
            versioning: optional_versioning(change.desired, change.address)?,
            max_size: optional_integer(change.desired, "max_size", change.address)?,
            tags: wire_tags(change.desired, change.address)?,
        };
        let mut mask = Vec::new();
        if patch.default_storage_class.is_some() {
            mask.push("defaultStorageClass");
        }
        if patch.versioning.is_some() {
            mask.push("versioning");
        }
        if patch.max_size.is_some() {
            mask.push("maxSize");
        }
        if change.desired.get("tags").is_some() {
            mask.push("tags");
        }
        if mask.is_empty() {
            return Err(ProviderError::Declaration {
                address: change.address.clone(),
                problem: "an update with nothing the storage API can change".to_owned(),
            });
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
        Ok(())
    }

    /// Applies a bucket change, and reports what it became. The storage API
    /// names a bucket by its name, which is also its id.
    fn act_bucket(
        &self,
        change: &Prepared<'_>,
        headers: &[(&str, &str)],
        name: &str,
    ) -> Result<Concluded, ProviderError> {
        let existing = self
            .list_buckets(headers, change.address)?
            .into_iter()
            .find(|bucket| bucket.name == name);

        match change.kind {
            Sending::Delete => {
                if let Some(bucket) = existing {
                    let url = format!("{}/buckets/{}", self.storage, bucket.name);
                    world(change.address, self.api.delete(&url, headers))?;
                }
                Ok(Concluded::Gone)
            }
            Sending::Replace => {
                if let Some(bucket) = existing {
                    let url = format!("{}/buckets/{}", self.storage, bucket.name);
                    world(change.address, self.api.delete(&url, headers))?;
                }
                self.create_bucket(change, headers)?;
                self.read_bucket(change, headers, name)
            }
            Sending::Update => {
                existing.ok_or_else(|| ProviderError::World {
                    address: change.address.clone(),
                    problem: "the bucket to update is not there".to_owned(),
                })?;
                self.update_bucket(change, headers, name)?;
                self.read_bucket(change, headers, name)
            }
            Sending::Create => {
                self.create_bucket(change, headers)?;
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
}

/// One tag on a bucket, as the API answers with it.
#[derive(Debug, Deserialize, Serialize)]
struct WireTag {
    key: String,
    value: String,
}

/// The answer to a bucket list call.
#[derive(Debug, Deserialize)]
struct BucketList {
    buckets: Vec<WireBucket>,
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
    #[serde(rename = "tags", skip_serializing_if = "Vec::is_empty")]
    tags: Vec<WireTag>,
}

/// What a bucket update call is asked with. The update mask travels in the
/// URL, as the storage API asks; the body carries only what the mask names.
#[derive(Debug, Serialize, Deserialize)]
struct PatchBucket {
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
    #[serde(rename = "tags", skip_serializing_if = "Vec::is_empty")]
    tags: Vec<WireTag>,
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

/// A declared versioning word, in the API's spelling.
fn wire_versioning(declared: &str) -> Option<&'static str> {
    match declared {
        "enabled" => Some("VERSIONING_ENABLED"),
        "suspended" => Some("VERSIONING_SUSPENDED"),
        "disabled" => Some("VERSIONING_DISABLED"),
        _ => None,
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

/// The versioning a declaration manages, in the API's spelling, or `None`
/// where the declaration leaves it alone.
fn optional_versioning(
    desired: &Attributes,
    address: &Address,
) -> Result<Option<String>, ProviderError> {
    match desired.get("versioning") {
        None => Ok(None),
        Some(Value::Text(text)) => match wire_versioning(text) {
            Some(wire) => Ok(Some(wire.to_owned())),
            None => Err(ProviderError::Declaration {
                address: address.clone(),
                problem: format!(
                    "versioning is {text}; it is one of enabled, suspended and disabled"
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
        versioning: optional_versioning(desired, address)?,
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
                    let list = self.list_buckets(&headers, &address)?;
                    for reading in readings {
                        if let Some(bucket) = list
                            .iter()
                            .find(|bucket| bucket.name == wire_name(reading.address()))
                        {
                            found = found.with(
                                reading.address().clone(),
                                bucket_attributes(bucket),
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

/// The resource kind a URL addresses: `instances`, `networks`, `subnets` or
/// `buckets`.
fn list_kind(url: &str) -> &str {
    if url.contains("/buckets") {
        "buckets"
    } else if url.contains("/networks") {
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
            "buckets" => {
                let buckets = self.buckets.lock().map_err(|_| poisoned())?;
                let mut listed: Vec<&WireBucket> = buckets.values().collect();
                listed.sort_by(|left, right| left.name.cmp(&right.name));
                serde_json::to_string(&serde_json::json!({ "buckets": listed }))
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
            "buckets" => {
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
        // A bucket is corrected in place through its update mask; an instance
        // through its resources. Networks and subnets replace rather than
        // patch.
        if list_kind(url) == "buckets" {
            let name = url
                .split_once('?')
                .map_or(url, |(before, _)| before)
                .rsplit('/')
                .next()
                .unwrap_or("");
            let mask: Vec<&str> = url
                .split_once('?')
                .and_then(|(_, after)| after.strip_prefix("updateMask="))
                .map_or_else(Vec::new, |listed| listed.split(',').collect());
            let patch: PatchBucket = serde_json::from_str(body)
                .map_err(|error| ApiError::Transport(error.to_string()))?;
            let mut buckets = self.buckets.lock().map_err(|_| poisoned())?;
            if let Some(bucket) = buckets.get_mut(name) {
                if mask.contains(&"defaultStorageClass") {
                    bucket.default_storage_class = patch.default_storage_class;
                }
                if mask.contains(&"versioning") {
                    bucket.versioning = patch.versioning;
                }
                if mask.contains(&"maxSize") {
                    bucket.max_size = patch.max_size;
                }
                if mask.contains(&"tags") {
                    bucket.tags = patch.tags;
                }
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
            // A bucket is named by its name, which is also its id.
            "buckets" => {
                self.buckets
                    .lock()
                    .map_err(|_| poisoned())?
                    .retain(|_, bucket| bucket.name != id);
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
