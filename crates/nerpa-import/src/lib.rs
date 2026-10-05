//! A Terraform state, read into nerpa.
//!
//! Stage 3's way in from Terraform: an environment that already exists is read
//! out of its state file rather than recreated. The import maps the resources
//! nerpa knows to addresses and attributes, and names every secret the old
//! state held in clear text — Terraform's state is flat and unencrypted, and a
//! password in it is a password anybody with the file can read.

use std::collections::BTreeMap;

use nerpa_core::Value;
use serde::Deserialize;

/// A Terraform state document, in the flatmap `attributes` shape.
#[derive(Debug, Deserialize)]
pub struct TfState {
    /// The state format version.
    pub version: i64,
    /// The resources Terraform manages.
    pub resources: Vec<TfResource>,
}

/// One resource in the state.
#[derive(Debug, Deserialize)]
pub struct TfResource {
    /// The Terraform resource type, e.g. `yandex_compute_instance`.
    #[serde(rename = "type")]
    pub resource_type: String,
    /// The resource's name in the Terraform configuration.
    pub name: String,
    /// The instances of the resource, usually one.
    pub instances: Vec<TfInstance>,
}

/// One instance of a resource: its attributes, as Terraform's flatmap.
#[derive(Debug, Deserialize)]
pub struct TfInstance {
    /// The flatmap of attribute name to value, every value a string.
    pub attributes: BTreeMap<String, String>,
}

/// A resource as nerpa will declare it.
#[derive(Debug, PartialEq, Eq)]
pub struct Imported {
    /// The nerpa address, e.g. `yandex:instance.db`.
    pub address: String,
    /// Its attributes, as name → nerpa value.
    pub attributes: BTreeMap<String, Value>,
}

/// A secret the state held in clear text.
#[derive(Debug, PartialEq, Eq)]
pub struct Secret {
    /// The resource it belongs to, as `type "name"`.
    pub resource: String,
    /// The attribute whose name says it is a secret.
    pub attribute: String,
}

/// The whole import.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Import {
    /// The resources nerpa will declare.
    pub resources: Vec<Imported>,
    /// Every secret found in clear text.
    pub secrets: Vec<Secret>,
}

/// Words that mark an attribute as a secret when they appear in its name.
const SECRET_WORDS: &[&str] = &[
    "password",
    "private_key",
    "token",
    "secret",
    "access_key",
    "api_key",
    "ssh_key",
    "credential",
];

impl Import {
    /// Reads a state into nerpa: maps the resources it knows, and names every
    /// secret found in clear text.
    ///
    /// # Errors
    ///
    /// Returns a message when a resource nerpa maps is missing an attribute it
    /// needs.
    pub fn read(state: &TfState) -> Result<Self, String> {
        let mut import = Self::default();
        for resource in &state.resources {
            let known = matches!(
                resource.resource_type.as_str(),
                "yandex_compute_instance" | "yandex_vpc_network" | "yandex_vpc_subnet"
            );
            for instance in &resource.instances {
                if known {
                    import.resources.push(map(resource, instance)?);
                }
                for attribute in instance.attributes.keys() {
                    if secret_name(attribute) {
                        import.secrets.push(Secret {
                            resource: format!("{} \"{}\"", resource.resource_type, resource.name),
                            attribute: attribute.clone(),
                        });
                    }
                }
            }
        }
        Ok(import)
    }

    /// The configuration that declares what was imported.
    #[must_use]
    pub fn render(&self) -> String {
        use std::fmt::Write as _;
        let mut out = String::new();
        for resource in &self.resources {
            if resource.attributes.is_empty() {
                let _ = writeln!(out, "resource(\"{}\")", resource.address);
                continue;
            }
            let _ = writeln!(out, "resource(\"{}\", {{", resource.address);
            for (name, value) in &resource.attributes {
                let _ = writeln!(out, "    \"{name}\": {},", render_value(value));
            }
            let _ = writeln!(out, "}})");
        }
        out
    }
}

/// Maps one instance of a known resource to a nerpa declaration.
fn map(resource: &TfResource, instance: &TfInstance) -> Result<Imported, String> {
    let attributes = &instance.attributes;
    let name = attributes.get("name").ok_or_else(|| {
        format!(
            "{} \"{}\": no `name` attribute",
            resource.resource_type, resource.name
        )
    })?;
    match resource.resource_type.as_str() {
        "yandex_compute_instance" => {
            let mut declared = BTreeMap::new();
            declared.insert(
                "zone_id".to_owned(),
                Value::Text(text(attributes, "zone", resource)?),
            );
            declared.insert(
                "platform_id".to_owned(),
                Value::Text(text(attributes, "platform_id", resource)?),
            );
            declared.insert(
                "cores".to_owned(),
                Value::Integer(integer(attributes, "resources.0.cores", resource)?),
            );
            declared.insert(
                "memory_gb".to_owned(),
                Value::Integer(integer(attributes, "resources.0.memory", resource)?),
            );
            Ok(Imported {
                address: format!("yandex:instance.{name}"),
                attributes: declared,
            })
        }
        "yandex_vpc_network" => Ok(Imported {
            address: format!("yandex:network.{name}"),
            attributes: BTreeMap::new(),
        }),
        "yandex_vpc_subnet" => {
            let mut declared = BTreeMap::new();
            declared.insert(
                "network_id".to_owned(),
                Value::Text(text(attributes, "network_id", resource)?),
            );
            declared.insert(
                "zone_id".to_owned(),
                Value::Text(text(attributes, "zone", resource)?),
            );
            declared.insert(
                "v4_cidr_blocks".to_owned(),
                Value::List(
                    list(attributes, "v4_cidr_blocks", resource)?
                        .into_iter()
                        .map(Value::Text)
                        .collect(),
                ),
            );
            Ok(Imported {
                address: format!("yandex:subnet.{name}"),
                attributes: declared,
            })
        }
        _ => Err(format!(
            "{}: not a resource nerpa imports",
            resource.resource_type
        )),
    }
}

/// A required text attribute of a resource.
fn text(
    attributes: &BTreeMap<String, String>,
    key: &str,
    resource: &TfResource,
) -> Result<String, String> {
    attributes.get(key).cloned().ok_or_else(|| {
        format!(
            "{} \"{}\": `{key}` is missing",
            resource.resource_type, resource.name
        )
    })
}

/// A required integer attribute of a resource.
fn integer(
    attributes: &BTreeMap<String, String>,
    key: &str,
    resource: &TfResource,
) -> Result<i64, String> {
    let value = text(attributes, key, resource)?;
    value.parse().map_err(|_| {
        format!(
            "{} \"{}\": `{key}` is not an integer",
            resource.resource_type, resource.name
        )
    })
}

/// A flatmap list: `key.#` counts, `key.{i}` names each entry.
fn list(
    attributes: &BTreeMap<String, String>,
    key: &str,
    resource: &TfResource,
) -> Result<Vec<String>, String> {
    let count = integer(attributes, &format!("{key}.#"), resource)?;
    let mut values = Vec::new();
    for index in 0..count {
        values.push(text(attributes, &format!("{key}.{index}"), resource)?);
    }
    Ok(values)
}

/// Whether an attribute's name says it is a secret.
fn secret_name(key: &str) -> bool {
    let lower = key.to_lowercase();
    SECRET_WORDS.iter().any(|word| lower.contains(word))
}

/// A nerpa value, written as a Starlark literal.
fn render_value(value: &Value) -> String {
    match value {
        Value::Text(text) => format!("{text:?}"),
        Value::Integer(integer) => integer.to_string(),
        Value::Boolean(boolean) => boolean.to_string(),
        Value::List(values) => {
            let entries: Vec<String> = values.iter().map(render_value).collect();
            format!("[{}]", entries.join(", "))
        }
        _ => "()".to_owned(),
    }
}
