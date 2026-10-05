//! A Terraform state, read into nerpa.
//!
//! Stage 3's way in from Terraform: an environment that already exists is read
//! out of its state file rather than recreated. The import maps the resources
//! nerpa knows to addresses and attributes, and names every secret the old
//! state held in clear text — Terraform's state is flat and unencrypted, and a
//! password in it is a password anybody with the file can read.

use std::collections::{BTreeMap, BTreeSet};

use nerpa_core::Value;
use nerpa_core::secret::Recorded;
use nerpa_core::state::{Record, State};
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
    /// `managed` or `data`: a data source is read, not managed.
    pub mode: String,
    /// The instances of the resource, usually one.
    pub instances: Vec<TfInstance>,
}

/// One instance of a resource: its nested attributes.
#[derive(Debug, Deserialize)]
pub struct TfInstance {
    /// The attributes, nested objects and lists, values of any JSON kind.
    pub attributes: serde_json::Value,
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
///
/// `token` is deliberately absent: it also names the metadata service's
/// `*_http_token` settings, which are integers saying whether the service asks
/// for one, not a token itself. A real token in state is caught by its own
/// name — `access_token`, `iam_token` — when it is written.
const SECRET_WORDS: &[&str] = &[
    "password",
    "private_key",
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
            let managed = resource.mode != "data";
            let known = matches!(
                resource.resource_type.as_str(),
                "yandex_compute_instance" | "yandex_vpc_network" | "yandex_vpc_subnet"
            );
            for instance in &resource.instances {
                if managed && known {
                    import.resources.push(map(resource, instance)?);
                }
                for attribute in secrets_in(&instance.attributes) {
                    import.secrets.push(Secret {
                        resource: format!("{} \"{}\"", resource.resource_type, resource.name),
                        attribute,
                    });
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

    /// The state that records what was imported: each resource, with its
    /// attributes as plain values. No secret among them — the import names
    /// secrets rather than carrying them into state.
    ///
    /// # Errors
    ///
    /// Returns a message when an address the import produced does not parse,
    /// which would mean the mapping named something the address grammar rejects.
    pub fn state(&self) -> Result<State, String> {
        let mut state = State::new();
        for resource in &self.resources {
            let address: nerpa_core::Address = resource
                .address
                .parse()
                .map_err(|error| format!("{}: {error}", resource.address))?;
            let recording = Recorded::all(resource.attributes.clone());
            state.record(address, Record::new(recording, BTreeSet::new()));
        }
        Ok(state)
    }
}

/// Maps one instance of a known resource to a nerpa declaration.
fn map(resource: &TfResource, instance: &TfInstance) -> Result<Imported, String> {
    let attributes = &instance.attributes;
    let name = text(attributes, "name", resource)?;
    match resource.resource_type.as_str() {
        "yandex_compute_instance" => {
            let resources = attributes
                .get("resources")
                .and_then(serde_json::Value::as_array)
                .and_then(|values| values.first())
                .ok_or_else(|| {
                    format!(
                        "{} \"{}\": `resources` is missing or empty",
                        resource.resource_type, resource.name
                    )
                })?;
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
                Value::Integer(integer(resources, "cores", resource)?),
            );
            declared.insert(
                "memory_gb".to_owned(),
                Value::Integer(integer(resources, "memory", resource)?),
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
                    strings(attributes, "v4_cidr_blocks", resource)?
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
    attributes: &serde_json::Value,
    key: &str,
    resource: &TfResource,
) -> Result<String, String> {
    attributes
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            format!(
                "{} \"{}\": `{key}` is missing or not text",
                resource.resource_type, resource.name
            )
        })
}

/// A required integer attribute of a resource.
fn integer(
    attributes: &serde_json::Value,
    key: &str,
    resource: &TfResource,
) -> Result<i64, String> {
    attributes
        .get(key)
        .and_then(serde_json::Value::as_i64)
        .ok_or_else(|| {
            format!(
                "{} \"{}\": `{key}` is missing or not an integer",
                resource.resource_type, resource.name
            )
        })
}

/// A required list-of-text attribute of a resource.
fn strings(
    attributes: &serde_json::Value,
    key: &str,
    resource: &TfResource,
) -> Result<Vec<String>, String> {
    attributes
        .get(key)
        .and_then(serde_json::Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .ok_or_else(|| {
            format!(
                "{} \"{}\": `{key}` is missing or not a list of text",
                resource.resource_type, resource.name
            )
        })
}

/// Whether an attribute's name says it is a secret.
fn secret_name(key: &str) -> bool {
    let lower = key.to_lowercase();
    SECRET_WORDS.iter().any(|word| lower.contains(word))
}

/// Every attribute name in a nested value whose name says it is a secret.
fn secrets_in(value: &serde_json::Value) -> Vec<String> {
    let mut found = Vec::new();
    walk_for_secrets(value, &mut found);
    found
}

/// Walks a nested value, collecting secret-named keys.
fn walk_for_secrets(value: &serde_json::Value, found: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, value) in map {
                if secret_name(key) {
                    found.push(key.clone());
                }
                walk_for_secrets(value, found);
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                walk_for_secrets(value, found);
            }
        }
        _ => {}
    }
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
