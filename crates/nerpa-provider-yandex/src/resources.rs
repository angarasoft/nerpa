//! The mapping between a declaration's attributes and the Yandex API shapes.
//!
//! One function per direction per kind: attributes to a create or update
//! request, and a listed resource back to attributes. The provider's `read`
//! and `conclude` share the read-back mapping, so what a change reports is
//! exactly what the next run reads — the reason a second plan is empty.

use std::collections::HashMap;

use nerpa_core::address::Address;
use nerpa_core::provider::{Attributes, ProviderError};
use nerpa_core::value::Value;

use crate::yandex::cloud::compute::v1::{
    AttachedDiskSpec, CreateDiskRequest, CreateInstanceRequest, Disk, Instance,
    NetworkInterfaceSpec, ResourcesSpec, UpdateDiskRequest, UpdateInstanceRequest,
    attached_disk_spec, create_disk_request, disk,
};
use crate::yandex::cloud::storage::v1::{Bucket, CreateBucketRequest};
use crate::yandex::cloud::vpc::v1::{
    CreateNetworkRequest, CreateSecurityGroupRequest, CreateSubnetRequest, Network, SecurityGroup,
    Subnet,
};
use prost_types::FieldMask;

/// A gibibyte, the unit sizes are given in.
const GIB: i64 = 1_073_741_824;

/// A text attribute, wherever a declared attribute of that shape is read.
pub(crate) fn text(attributes: &Attributes, name: &str) -> Option<String> {
    match attributes.get(name)? {
        Value::Text(text) => Some(text.clone()),
        _ => None,
    }
}

/// An integer attribute, wherever a declared attribute of that shape is read.
pub(crate) fn integer(attributes: &Attributes, name: &str) -> Option<i64> {
    match attributes.get(name)? {
        Value::Integer(integer) => Some(*integer),
        _ => None,
    }
}

/// A list of text, wherever a declared attribute of that shape is read.
pub(crate) fn list_of_text(attributes: &Attributes, name: &str) -> Option<Vec<String>> {
    match attributes.get(name)? {
        Value::List(values) => values
            .iter()
            .map(|value| match value {
                Value::Text(text) => Some(text.clone()),
                _ => None,
            })
            .collect(),
        _ => None,
    }
}

/// A text attribute that must be there for the change to mean anything.
pub(crate) fn required_text(
    attributes: &Attributes,
    name: &str,
    address: &Address,
) -> Result<String, ProviderError> {
    text(attributes, name).ok_or_else(|| ProviderError::Declaration {
        address: address.clone(),
        problem: format!("needs a `{name}`"),
    })
}

/// An integer attribute that must be there for the change to mean anything.
pub(crate) fn required_integer(
    attributes: &Attributes,
    name: &str,
    address: &Address,
) -> Result<i64, ProviderError> {
    integer(attributes, name).ok_or_else(|| ProviderError::Declaration {
        address: address.clone(),
        problem: format!("needs a `{name}`"),
    })
}

/// What a network looks like, as declared attributes plus the id the cloud
/// assigned.
pub(crate) fn network_attributes(network: &Network) -> Attributes {
    let mut attributes = Attributes::new();
    attributes.insert(
        "folder_id".to_owned(),
        Value::Text(network.folder_id.clone()),
    );
    attributes.insert("name".to_owned(), Value::Text(network.name.clone()));
    attributes.insert("id".to_owned(), Value::Text(network.id.clone()));
    attributes
}

/// What a subnet looks like, as declared attributes plus the id the cloud
/// assigned.
pub(crate) fn subnet_attributes(subnet: &Subnet) -> Attributes {
    let mut attributes = Attributes::new();
    attributes.insert(
        "folder_id".to_owned(),
        Value::Text(subnet.folder_id.clone()),
    );
    attributes.insert("name".to_owned(), Value::Text(subnet.name.clone()));
    attributes.insert("id".to_owned(), Value::Text(subnet.id.clone()));
    attributes.insert("zone".to_owned(), Value::Text(subnet.zone_id.clone()));
    attributes.insert(
        "network_id".to_owned(),
        Value::Text(subnet.network_id.clone()),
    );
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

/// What a security group looks like, as declared attributes plus the id the
/// cloud assigned.
pub(crate) fn security_group_attributes(group: &SecurityGroup) -> Attributes {
    let mut attributes = Attributes::new();
    attributes.insert("folder_id".to_owned(), Value::Text(group.folder_id.clone()));
    attributes.insert("name".to_owned(), Value::Text(group.name.clone()));
    attributes.insert("id".to_owned(), Value::Text(group.id.clone()));
    attributes.insert(
        "network_id".to_owned(),
        Value::Text(group.network_id.clone()),
    );
    attributes
}

/// What a disk looks like, as declared attributes plus the id the cloud
/// assigned. A disk made blank has no `image_id`, exactly as it was declared.
pub(crate) fn disk_attributes(disk: &Disk) -> Attributes {
    let mut attributes = Attributes::new();
    attributes.insert("folder_id".to_owned(), Value::Text(disk.folder_id.clone()));
    attributes.insert("name".to_owned(), Value::Text(disk.name.clone()));
    attributes.insert("id".to_owned(), Value::Text(disk.id.clone()));
    attributes.insert("type".to_owned(), Value::Text(disk.type_id.clone()));
    attributes.insert("zone".to_owned(), Value::Text(disk.zone_id.clone()));
    attributes.insert("size".to_owned(), Value::Integer(disk.size));
    if let Some(disk::Source::SourceImageId(image_id)) = &disk.source {
        attributes.insert("image_id".to_owned(), Value::Text(image_id.clone()));
    }
    attributes
}

/// What an instance looks like, as declared attributes plus the id the cloud
/// assigned. The boot disk is looked up separately: an instance names its boot
/// disk by id, and the declaration names it by the image it was made from.
pub(crate) fn instance_attributes(instance: &Instance, boot_disk: &Disk) -> Attributes {
    let mut attributes = Attributes::new();
    attributes.insert(
        "folder_id".to_owned(),
        Value::Text(instance.folder_id.clone()),
    );
    attributes.insert("name".to_owned(), Value::Text(instance.name.clone()));
    attributes.insert("id".to_owned(), Value::Text(instance.id.clone()));
    attributes.insert("zone".to_owned(), Value::Text(instance.zone_id.clone()));
    attributes.insert(
        "platform_id".to_owned(),
        Value::Text(instance.platform_id.clone()),
    );
    if let Some(resources) = &instance.resources {
        attributes.insert("cores".to_owned(), Value::Integer(resources.cores));
        attributes.insert("memory".to_owned(), Value::Integer(resources.memory));
        attributes.insert(
            "core_fraction".to_owned(),
            Value::Integer(resources.core_fraction),
        );
    }
    if let Some(disk::Source::SourceImageId(image_id)) = &boot_disk.source {
        attributes.insert("boot_disk".to_owned(), Value::Text(image_id.clone()));
    }
    if let Some(user_data) = instance.metadata.get("user-data") {
        attributes.insert("user_data".to_owned(), Value::Text(user_data.clone()));
    }
    if let Some(interface) = instance.network_interfaces.first() {
        attributes.insert(
            "subnet".to_owned(),
            Value::Text(interface.subnet_id.clone()),
        );
    }
    attributes
}

/// The create request a declared network becomes.
pub(crate) fn create_network(
    desired: &Attributes,
    address: &Address,
) -> Result<CreateNetworkRequest, ProviderError> {
    Ok(CreateNetworkRequest {
        folder_id: required_text(desired, "folder_id", address)?,
        name: required_text(desired, "name", address)?,
        ..Default::default()
    })
}

/// The create request a declared subnet becomes.
pub(crate) fn create_subnet(
    desired: &Attributes,
    address: &Address,
) -> Result<CreateSubnetRequest, ProviderError> {
    let v4_cidr_blocks =
        list_of_text(desired, "v4_cidr_blocks").ok_or_else(|| ProviderError::Declaration {
            address: address.clone(),
            problem: "needs a `v4_cidr_blocks`".to_owned(),
        })?;
    Ok(CreateSubnetRequest {
        folder_id: required_text(desired, "folder_id", address)?,
        name: required_text(desired, "name", address)?,
        zone_id: required_text(desired, "zone", address)?,
        network_id: required_text(desired, "network_id", address)?,
        v4_cidr_blocks,
        ..Default::default()
    })
}

/// The create request a declared security group becomes.
pub(crate) fn create_security_group(
    desired: &Attributes,
    address: &Address,
) -> Result<CreateSecurityGroupRequest, ProviderError> {
    Ok(CreateSecurityGroupRequest {
        folder_id: required_text(desired, "folder_id", address)?,
        name: required_text(desired, "name", address)?,
        network_id: required_text(desired, "network_id", address)?,
        ..Default::default()
    })
}

/// The create request a declared disk becomes. A disk without `image_id` is
/// made blank, which is what the declaration asked for.
pub(crate) fn create_disk(
    desired: &Attributes,
    address: &Address,
) -> Result<CreateDiskRequest, ProviderError> {
    Ok(CreateDiskRequest {
        folder_id: required_text(desired, "folder_id", address)?,
        name: required_text(desired, "name", address)?,
        type_id: required_text(desired, "type", address)?,
        zone_id: required_text(desired, "zone", address)?,
        size: required_integer(desired, "size", address)?,
        source: text(desired, "image_id").map(create_disk_request::Source::ImageId),
        ..Default::default()
    })
}

/// The create request a declared instance becomes.
pub(crate) fn create_instance(
    desired: &Attributes,
    address: &Address,
) -> Result<CreateInstanceRequest, ProviderError> {
    let boot_disk = required_text(desired, "boot_disk", address)?;
    // A subnet is optional: an instance can be declared without a network,
    // which is also what a recording takes when the cloud's vpc quota is
    // spent.
    let subnet = text(desired, "subnet");
    // Fractional cores exist on some platforms; a declaration names the
    // fraction, and full cores are the default.
    let core_fraction = integer(desired, "core_fraction").unwrap_or(100);
    let metadata = text(desired, "user_data").map_or_else(HashMap::new, |user_data| {
        HashMap::from([("user-data".to_owned(), user_data)])
    });
    Ok(CreateInstanceRequest {
        folder_id: required_text(desired, "folder_id", address)?,
        name: required_text(desired, "name", address)?,
        zone_id: required_text(desired, "zone", address)?,
        platform_id: required_text(desired, "platform_id", address)?,
        resources_spec: Some(ResourcesSpec {
            memory: required_integer(desired, "memory", address)?,
            cores: required_integer(desired, "cores", address)?,
            core_fraction,
            gpus: 0,
        }),
        boot_disk_spec: Some(AttachedDiskSpec {
            mode: i32::from(attached_disk_spec::Mode::ReadWrite),
            auto_delete: true,
            disk: Some(attached_disk_spec::Disk::DiskSpec(
                attached_disk_spec::DiskSpec {
                    // A size must be on the wire: the live API refuses a
                    // missing field, and prost omits a zero. Ten gibibytes
                    // is the image's own minimum.
                    size: 10 * GIB,
                    source: Some(attached_disk_spec::disk_spec::Source::ImageId(boot_disk)),
                    ..Default::default()
                },
            )),
            ..Default::default()
        }),
        metadata,
        network_interface_specs: subnet
            .into_iter()
            .map(|subnet_id| NetworkInterfaceSpec {
                subnet_id,
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    })
}

/// The update a cores or memory change becomes. Yandex only accepts it while
/// the instance is stopped, so the provider stops and starts around it.
pub(crate) fn update_instance(
    instance_id: String,
    desired: &Attributes,
    address: &Address,
) -> Result<UpdateInstanceRequest, ProviderError> {
    Ok(UpdateInstanceRequest {
        instance_id,
        update_mask: Some(FieldMask {
            paths: vec!["resources_spec".to_owned()],
        }),
        resources_spec: Some(ResourcesSpec {
            memory: required_integer(desired, "memory", address)?,
            cores: required_integer(desired, "cores", address)?,
            core_fraction: integer(desired, "core_fraction").unwrap_or(100),
            gpus: 0,
        }),
        ..Default::default()
    })
}

/// The update a disk growth becomes. The planner refuses a shrink before this
/// is ever built.
pub(crate) fn update_disk(
    disk_id: String,
    desired: &Attributes,
    address: &Address,
) -> Result<UpdateDiskRequest, ProviderError> {
    Ok(UpdateDiskRequest {
        disk_id,
        update_mask: Some(FieldMask {
            paths: vec!["size".to_owned()],
        }),
        size: required_integer(desired, "size", address)?,
        ..Default::default()
    })
}

/// The declared attribute names a kind changes in place. An `Update` that
/// differs in anything else is refused: the planner should not have sent it.
pub(crate) fn in_place_attributes(kind: &str) -> &'static [&'static str] {
    match kind {
        "instance" => &["cores", "memory"],
        "disk" => &["size"],
        _ => &[],
    }
}

/// The first declared attribute whose desired value differs from what was
/// read, if any. An update acts on the in-place set and refuses everything
/// else by name.
pub(crate) fn differing<'a>(current: &'a Attributes, desired: &'a Attributes) -> Option<&'a str> {
    desired
        .iter()
        .find(|(name, value)| current.get(name.as_str()) != Some(value))
        .map(|(name, _)| name.as_str())
}

/// A folder move is a change Yandex performs through a separate `Move` call
/// this provider does not implement; it is refused by name rather than
/// silently applied to nothing.
pub(crate) fn folder_moved(current: &Attributes, desired: &Attributes) -> bool {
    current.get("folder_id") != desired.get("folder_id")
}

/// The name a reading is looked up by: the identity attribute, `name`.
pub(crate) fn lookup(
    locator: &Attributes,
    address: &Address,
) -> Result<(String, String), ProviderError> {
    let folder_id = required_text(locator, "folder_id", address)?;
    let name = required_text(locator, "name", address)?;
    Ok((folder_id, name))
}

/// What a bucket looks like, as declared attributes plus the id the cloud
/// assigned — the bucket's name, which Object Storage makes the id too.
pub(crate) fn bucket_attributes(bucket: &Bucket) -> Attributes {
    let mut attributes = Attributes::new();
    attributes.insert(
        "folder_id".to_owned(),
        Value::Text(bucket.folder_id.clone()),
    );
    attributes.insert("name".to_owned(), Value::Text(bucket.name.clone()));
    attributes.insert("id".to_owned(), Value::Text(bucket.id.clone()));
    attributes
}

/// The create request a declared bucket becomes.
pub(crate) fn create_bucket(
    desired: &Attributes,
    address: &Address,
) -> Result<CreateBucketRequest, ProviderError> {
    Ok(CreateBucketRequest {
        name: required_text(desired, "name", address)?,
        folder_id: required_text(desired, "folder_id", address)?,
        ..Default::default()
    })
}
