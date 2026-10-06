//! The manifest a WASM provider ships beside its component.
//!
//! The manifest is data, not code: what a provider owns and how its attributes
//! behave, written down so the engine can read it without instantiating the
//! component (adr-2026-0004, "schemas are harvested as data").
//!
//! ```toml
//! name = "example"
//!
//! [kind.file]
//! immutable = ["path"]
//! bounded = { path = 1048576 }
//! identity = { attribute = "path", namespace = "path" }
//!
//! [kind.instance]
//! creates_node = true
//! ```
//!
//! Every refinement is optional; the default is "converged" — an attribute that
//! differs is corrected in place.

use std::collections::BTreeMap;

use nerpa_core::Schema;
use nerpa_core::address::{Kind, Provider};
use serde::Deserialize;

/// The manifest, read from the TOML beside the component.
#[derive(Debug, Deserialize)]
pub struct Manifest {
    /// The provider name, as written in an address: `example` in
    /// `example:file.x@node`.
    name: String,
    /// One table per resource kind, keyed by the kind name.
    #[serde(default)]
    kind: BTreeMap<String, KindManifest>,
}

/// One resource kind and how its attributes behave.
#[derive(Debug, Default, Deserialize)]
struct KindManifest {
    /// Changing one of these means replacing the resource, not editing it.
    #[serde(default)]
    immutable: Vec<String>,
    /// Describes how the resource was made; a difference is reported, never acted on.
    #[serde(default)]
    at_birth: Vec<String>,
    /// File modes, written in octal.
    #[serde(default)]
    mode: Vec<String>,
    /// The most, in bytes, an attribute may carry to a machine in one operation.
    #[serde(default)]
    bounded: BTreeMap<String, usize>,
    /// The attribute that names the thing on its machine, and the namespace.
    #[serde(default)]
    identity: Option<Identity>,
    /// This kind never reacts to a change elsewhere.
    #[serde(default)]
    unreactive: bool,
    /// This kind creates a node named by the resource's name.
    #[serde(default)]
    creates_node: bool,
}

/// What names a thing on its machine, within a namespace several kinds share.
#[derive(Debug, Deserialize)]
struct Identity {
    attribute: String,
    namespace: String,
}

impl Manifest {
    /// Reads a manifest from its TOML text.
    ///
    /// # Errors
    ///
    /// Returns the parser's message when the text is not a manifest.
    pub fn read(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|error| error.to_string())
    }

    /// The provider name, validated against the address grammar.
    ///
    /// # Errors
    ///
    /// Returns what the grammar refused, when the name is not a valid provider
    /// name.
    pub fn name(&self) -> Result<Provider, String> {
        Provider::new(&self.name).map_err(|error| error.to_string())
    }

    /// The schema this manifest declares, keyed by kind.
    ///
    /// # Errors
    ///
    /// Returns what the grammar refused, when a kind name is not a valid kind.
    pub fn schema(&self) -> Result<Schema, String> {
        let provider = self.name()?;
        let mut schema = Schema::new();
        for (kind_name, kind) in &self.kind {
            let address_kind = Kind::new(kind_name).map_err(|error| error.to_string())?;
            for attribute in &kind.immutable {
                schema =
                    schema.immutable(provider.clone(), address_kind.clone(), attribute.clone());
            }
            for attribute in &kind.at_birth {
                schema = schema.at_birth(provider.clone(), address_kind.clone(), attribute.clone());
            }
            for attribute in &kind.mode {
                schema = schema.mode(provider.clone(), address_kind.clone(), attribute.clone());
            }
            for (attribute, most) in &kind.bounded {
                schema = schema.bounded(
                    provider.clone(),
                    address_kind.clone(),
                    attribute.clone(),
                    *most,
                );
            }
            if let Some(identity) = &kind.identity {
                schema = schema.identity(
                    provider.clone(),
                    address_kind.clone(),
                    identity.attribute.clone(),
                    identity.namespace.clone(),
                );
            }
            if kind.unreactive {
                schema = schema.cannot_react(provider.clone(), address_kind.clone());
            }
            if kind.creates_node {
                schema = schema.creates_node(provider.clone(), address_kind.clone());
            }
        }
        Ok(schema)
    }
}
