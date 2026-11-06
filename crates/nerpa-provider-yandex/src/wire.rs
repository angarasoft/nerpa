//! The wire's own shapes: what the API answers with, what it is asked with,
//! the two int64 spellings, the pages of its lists, and the one vocabulary
//! both doors share.

use nerpa_core::address::Address;
use nerpa_core::provider::{Attributes, ProviderError};
use nerpa_core::value::Value;
use serde::{Deserialize, Serialize};

/// An instance, as the API answers with it.
#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct WireInstance {
    pub(crate) id: String,
    pub(crate) name: String,
    #[serde(rename = "zoneId")]
    pub(crate) zone_id: String,
    #[serde(rename = "platformId")]
    pub(crate) platform_id: String,
    pub(crate) resources: WireResources,
    #[serde(rename = "networkInterfaces", default)]
    pub(crate) network_interfaces: Vec<WireInterface>,
}

/// One network interface of an instance.
#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct WireInterface {
    #[serde(rename = "subnetId")]
    pub(crate) subnet_id: String,
}

/// What an instance is made of.
#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct WireResources {
    #[serde(with = "int64")]
    pub(crate) cores: i64,
    #[serde(with = "int64")]
    pub(crate) memory: i64,
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
pub(crate) struct InstanceList {
    #[serde(default)]
    pub(crate) instances: Vec<WireInstance>,
    /// The page after this one, where the folder holds more than a page.
    #[serde(rename = "nextPageToken", default)]
    pub(crate) next_page_token: Option<String>,
}

/// What a create call is asked with.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct CreateInstance {
    #[serde(rename = "folderId")]
    pub(crate) folder_id: String,
    pub(crate) name: String,
    #[serde(rename = "zoneId")]
    pub(crate) zone_id: String,
    #[serde(rename = "platformId")]
    pub(crate) platform_id: String,
    #[serde(rename = "resourcesSpec")]
    pub(crate) resources_spec: WireResources,
}

/// What an update call is asked with: the parts that may change in place.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct PatchInstance {
    #[serde(rename = "resourcesSpec")]
    pub(crate) resources_spec: WireResources,
}

/// A VPC network, as the API answers with it. A network has no state beyond its
/// name and its existence.
#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct WireNetwork {
    pub(crate) id: String,
    pub(crate) name: String,
}

/// The answer to a network list call. The field is absent where the folder
/// holds no networks: proto3 JSON omits a list that is empty, so an empty
/// folder answers `{}`.
#[derive(Debug, Deserialize)]
pub(crate) struct NetworkList {
    #[serde(default)]
    pub(crate) networks: Vec<WireNetwork>,
    /// The page after this one, where the folder holds more than a page.
    #[serde(rename = "nextPageToken", default)]
    pub(crate) next_page_token: Option<String>,
}

/// What a network create call is asked with.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct CreateNetwork {
    #[serde(rename = "folderId")]
    pub(crate) folder_id: String,
    pub(crate) name: String,
}

/// A subnet, as the API answers with it.
#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct WireSubnet {
    pub(crate) id: String,
    pub(crate) name: String,
    #[serde(rename = "networkId")]
    pub(crate) network_id: String,
    #[serde(rename = "zoneId")]
    pub(crate) zone_id: String,
    #[serde(rename = "v4CidrBlocks")]
    pub(crate) v4_cidr_blocks: Vec<String>,
}

/// The answer to a subnet list call. The field is absent where the folder
/// holds no subnets: proto3 JSON omits a list that is empty, so an empty
/// folder answers `{}`.
#[derive(Debug, Deserialize)]
pub(crate) struct SubnetList {
    #[serde(default)]
    pub(crate) subnets: Vec<WireSubnet>,
    /// The page after this one, where the folder holds more than a page.
    #[serde(rename = "nextPageToken", default)]
    pub(crate) next_page_token: Option<String>,
}

/// One page of a list, and what names the next: the API answers its lists in
/// pages, and a folder larger than one page is a folder like any other —
/// everything in it is part of the world the read answers for.
pub(crate) trait Pages {
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
pub(crate) struct CreateSubnet {
    #[serde(rename = "folderId")]
    pub(crate) folder_id: String,
    pub(crate) name: String,
    #[serde(rename = "networkId")]
    pub(crate) network_id: String,
    #[serde(rename = "zoneId")]
    pub(crate) zone_id: String,
    #[serde(rename = "v4CidrBlocks")]
    pub(crate) v4_cidr_blocks: Vec<String>,
}

/// A bucket, as the API answers with it. A bucket is named by its name, which
/// is unique across the whole platform and is also its id.
#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct WireBucket {
    pub(crate) name: String,
    #[serde(
        rename = "defaultStorageClass",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) default_storage_class: Option<String>,
    /// `VERSIONING_ENABLED`, `VERSIONING_SUSPENDED` or `VERSIONING_DISABLED`.
    /// The API omits a disabled one, because it is the zero value.
    #[serde(
        rename = "versioning",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) versioning: Option<String>,
    #[serde(
        rename = "maxSize",
        with = "optional_int64",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) max_size: Option<i64>,
    #[serde(rename = "tags", default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) tags: Vec<WireTag>,
    /// The object lock the bucket holds, where the full view carries one
    /// back: a WORM setting an update must carry as it found it, not reset.
    #[serde(
        rename = "objectLock",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) object_lock: Option<WireObjectLock>,
    /// The private-endpoint policy the bucket holds, where it carries one.
    #[serde(
        rename = "allowedPrivateEndpoints",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) allowed_private_endpoints: Option<WireAllowedPrivateEndpoints>,
    /// The website document the bucket holds, where it carries one. Carried
    /// through every update: the wire replaces what the body omits
    /// (adr-2026-082).
    #[serde(
        rename = "websiteSettings",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) website: Option<WireWebsite>,
    /// The anonymous access flags the bucket holds.
    #[serde(
        rename = "anonymousAccessFlags",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) anonymous: Option<WireAnonymous>,
}

/// The website document a bucket holds: the index page a request without a
/// name is served, and the page an error is served instead. The wire
/// replaces the document whole — see the carry in the update (adr-2026-082).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct WireWebsite {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) index: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
}

impl WireWebsite {
    /// Whether the document says anything to carry: a page to serve, or
    /// nothing but the wire's empty placeholder.
    #[must_use]
    pub(crate) fn carries(&self) -> bool {
        self.index.as_deref().is_some_and(|page| !page.is_empty())
            || self.error.as_deref().is_some_and(|page| !page.is_empty())
    }
}

/// The anonymous access flags a bucket holds: what the public may read
/// without a credential. The whole document — a flag the declaration does
/// not name reads as false (adr-2026-082).
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub(crate) struct WireAnonymous {
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub(crate) read: bool,
    #[serde(rename = "list", default, skip_serializing_if = "std::ops::Not::not")]
    pub(crate) list: bool,
    #[serde(
        rename = "configRead",
        default,
        skip_serializing_if = "std::ops::Not::not"
    )]
    pub(crate) config_read: bool,
}

/// One tag on a bucket, as the API answers with it.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct WireTag {
    pub(crate) key: String,
    pub(crate) value: String,
}

/// What a bucket create call is asked with. Only what the declaration manages
/// is carried: a field the declaration does not name is left to the cloud's
/// own default.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct CreateBucket {
    #[serde(rename = "folderId")]
    pub(crate) folder_id: String,
    pub(crate) name: String,
    #[serde(
        rename = "defaultStorageClass",
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) default_storage_class: Option<String>,
    #[serde(rename = "versioning", skip_serializing_if = "Option::is_none")]
    pub(crate) versioning: Option<String>,
    #[serde(
        rename = "maxSize",
        with = "optional_int64",
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) max_size: Option<i64>,
    #[serde(rename = "tags", default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) tags: Vec<WireTag>,
    #[serde(
        rename = "anonymousAccessFlags",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) anonymous: Option<WireAnonymous>,
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
pub(crate) struct PatchBucket {
    #[serde(rename = "versioning")]
    pub(crate) versioning: String,
    #[serde(
        rename = "defaultStorageClass",
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) default_storage_class: Option<String>,
    #[serde(
        rename = "maxSize",
        with = "optional_int64",
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) max_size: Option<i64>,
    #[serde(rename = "tags", default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) tags: Vec<WireTag>,
    #[serde(rename = "allowedPrivateEndpoints")]
    pub(crate) allowed_private_endpoints: WireAllowedPrivateEndpoints,
    #[serde(rename = "objectLock")]
    pub(crate) object_lock: WireObjectLock,
    /// Carried from the read where the bucket holds a site: the wire
    /// replaces what the body omits (adr-2026-082).
    #[serde(
        rename = "websiteSettings",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) website: Option<WireWebsite>,
    /// Carried only where declared: the wire leaves the flags alone when
    /// they are absent (adr-2026-082).
    #[serde(
        rename = "anonymousAccessFlags",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) anonymous: Option<WireAnonymous>,
}

/// The private-endpoint policy a bucket holds and an update carries: the
/// check, and whether the console's access is forced.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WireAllowedPrivateEndpoints {
    pub(crate) enabled: bool,
    #[serde(rename = "forceCloudConsoleAccess")]
    pub(crate) force_cloud_console_access: bool,
}

/// The object lock a bucket holds and an update carries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WireObjectLock {
    pub(crate) status: String,
}

/// The versioning a declaration may ask for: the two words the doors can
/// write. A bucket that was never versioned reads as disabled — a reported
/// state, not a declared one, because no door can unversion a bucket
/// (adr-2026-081). The three spellings of the two words — the declaration's,
/// the management API's and the S3 document's — are methods of this one
/// type, so a word that forgot validation cannot compile into a wrong
/// silence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Versioning {
    /// New versions are kept.
    Enabled,
    /// Old versions are kept, new ones are not.
    Suspended,
}

impl Versioning {
    /// The word as a declaration spells it.
    pub(crate) fn declared(self) -> &'static str {
        match self {
            Self::Enabled => "enabled",
            Self::Suspended => "suspended",
        }
    }

    /// The word as the management API spells it.
    pub(crate) fn management(self) -> &'static str {
        match self {
            Self::Enabled => "VERSIONING_ENABLED",
            Self::Suspended => "VERSIONING_SUSPENDED",
        }
    }

    /// The word as the S3 document spells it.
    pub(crate) fn s3(self) -> &'static str {
        match self {
            Self::Enabled => "Enabled",
            Self::Suspended => "Suspended",
        }
    }

    /// The versioning the management API's word says, where the word is one
    /// a declaration can hold.
    pub(crate) fn of_management(word: &str) -> Option<Self> {
        match word {
            "VERSIONING_ENABLED" => Some(Self::Enabled),
            "VERSIONING_SUSPENDED" => Some(Self::Suspended),
            _ => None,
        }
    }

    /// The versioning a declaration manages, or `None` where the declaration
    /// leaves it alone. Anything else is refused by name — including the
    /// word for the state no door can reach.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::Declaration`] where the declared value is
    /// neither text nor one of the two words.
    pub(crate) fn declared_of(
        desired: &Attributes,
        address: &Address,
    ) -> Result<Option<Self>, ProviderError> {
        match desired.get("versioning") {
            None => Ok(None),
            Some(Value::Text(text)) => match text.as_str() {
                "enabled" => Ok(Some(Self::Enabled)),
                "suspended" => Ok(Some(Self::Suspended)),
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
}
