//! The mapping between a declaration's attributes and the wire's shapes: one
//! function per direction per kind, the read-back shared with the next run's
//! read, and the helpers a declaration is read through.

use std::collections::BTreeMap;

use nerpa_core::address::Address;
use nerpa_core::provider::{Attributes, ProviderError};
use nerpa_core::value::Value;

use crate::wire::{
    CreateBucket, Versioning, WireBucket, WireInstance, WireResources, WireSubnet, WireTag,
};

/// One gibibyte: Yandex Cloud counts memory in bytes, a person counts in GiB.
const GIB: i64 = 1024 * 1024 * 1024;

/// The attributes a wire instance reads back as.
pub(crate) fn attributes(instance: &WireInstance) -> Attributes {
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
pub(crate) fn required_text(
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
pub(crate) fn required_integer(
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
pub(crate) fn resources(
    desired: &Attributes,
    address: &Address,
) -> Result<WireResources, ProviderError> {
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
pub(crate) fn subnet_attributes(subnet: &WireSubnet) -> Attributes {
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
pub(crate) fn bucket_attributes(bucket: &WireBucket) -> Attributes {
    let mut attributes = Attributes::new();
    if let Some(class) = &bucket.default_storage_class {
        attributes.insert(
            "default_storage_class".to_owned(),
            Value::Text(class.clone()),
        );
    }
    let versioning = bucket.versioning.as_deref().map_or_else(
        || "disabled".to_owned(),
        |word| {
            Versioning::of_management(word)
                .map_or_else(|| word, |versioning| versioning.declared())
                .to_owned()
        },
    );
    attributes.insert("versioning".to_owned(), Value::Text(versioning));
    if let Some(size) = bucket.max_size {
        attributes.insert("max_size".to_owned(), Value::Integer(size));
    }
    attributes.insert("tags".to_owned(), Value::Map(tag_map(&bucket.tags)));
    attributes
}

/// The tags a wire bucket holds, as a value map.
pub(crate) fn tag_map(tags: &[WireTag]) -> BTreeMap<String, Value> {
    tags.iter()
        .map(|tag| (tag.key.clone(), Value::Text(tag.value.clone())))
        .collect()
}

/// A required list-of-text attribute of a declaration.
pub(crate) fn required_list(
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
pub(crate) fn optional_text(
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
pub(crate) fn optional_integer(
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

/// The tags a declaration manages, in the API's spelling, or an empty list
/// where the declaration leaves them alone. An empty declared map clears the
/// tags, so it is carried as an empty list rather than as an absence.
pub(crate) fn wire_tags(
    desired: &Attributes,
    address: &Address,
) -> Result<Vec<WireTag>, ProviderError> {
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
pub(crate) fn create_bucket(
    desired: &Attributes,
    address: &Address,
    folder_id: &str,
) -> Result<CreateBucket, ProviderError> {
    Ok(CreateBucket {
        folder_id: folder_id.to_owned(),
        name: wire_name(address),
        default_storage_class: optional_text(desired, "default_storage_class", address)?,
        versioning: Versioning::declared_of(desired, address)?
            .map(|versioning| versioning.management().to_owned()),
        max_size: optional_integer(desired, "max_size", address)?,
        tags: wire_tags(desired, address)?,
    })
}

/// The name of a resource, as the wire knows it.
pub(crate) fn wire_name(address: &Address) -> String {
    address.name().as_str().to_owned()
}
