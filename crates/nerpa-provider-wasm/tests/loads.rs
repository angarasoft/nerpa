//! A WASM provider loads, and its manifest's kinds enter the schema.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::fs;
use std::path::PathBuf;

use nerpa_core::address::address;
use nerpa_core::provider::{Provider as ProviderTrait, Providers};
use nerpa_provider_wasm::{Manifest, Wasm};

/// A directory that removes itself, so a failing test cannot leave litter behind.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("nerpa-wasm-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("a scratch directory");
        Self(path)
    }

    fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// An empty component, built from a core module with nothing in it.
fn component() -> Vec<u8> {
    let core = wat::parse_str("(module)").expect("a core module");
    wit_component::ComponentEncoder::default()
        .module(&core)
        .expect("an encoder")
        .encode()
        .expect("a component")
}

#[test]
fn a_manifest_names_the_kinds_and_their_behaviour() {
    let manifest = Manifest::read(
        r#"
name = "example"

[kind.file]
immutable = ["path"]
bounded = { path = 1048576 }
identity = { attribute = "path", namespace = "path" }

[kind.instance]
creates_node = true
unreactive = true
"#,
    )
    .expect("a manifest");

    let schema = manifest.schema().expect("a schema");
    let file = address("example:file.x@localhost");
    let instance = address("example:instance.i@localhost");

    assert!(schema.forces_replacement(&file, "path"));
    assert!(!schema.forces_replacement(&file, "content"));
    assert_eq!(schema.bound(&file, "path"), Some(1_048_576));
    assert_eq!(schema.identity_of(&file), Some(("path", "path")));
    assert!(!schema.reacts(&instance));
    assert!(schema.creates_node_for(&instance));
}

#[test]
fn a_component_loads_beside_the_in_tree_providers() {
    let scratch = Scratch::new();
    let manifest = scratch.join("provider.toml");
    let component_path = scratch.join("provider.wasm");
    fs::write(
        &manifest,
        "name = \"example\"\n\n[kind.file]\nimmutable = [\"path\"]\n",
    )
    .expect("a manifest on disk");
    fs::write(&component_path, component()).expect("a component on disk");

    let wasm = Wasm::load(&manifest, &component_path).expect("a loaded provider");
    assert_eq!(wasm.name().as_str(), "example");

    let merged = Providers::new().with(Box::new(wasm)).schema();
    let file = address("example:file.x@localhost");
    assert!(merged.forces_replacement(&file, "path"));
}
