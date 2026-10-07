//! A WASM provider's operations are the ones that reach the machine.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use nerpa_core::address::address;
use nerpa_core::observed::Observed;
use nerpa_core::provider::{Prepared, Provider as ProviderTrait, Sending};
use nerpa_core::value::Value;
use nerpa_provider_wasm::Wasm;

/// A directory that removes itself.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("nerpa-wasm-creates-{}", std::process::id()));
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

/// The checked-in core module, wrapped into a component by the encoder.
fn component() -> Vec<u8> {
    let core = fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/guest_file_provider.wasm"
    ))
    .expect("the core module");
    wit_component::ComponentEncoder::default()
        .module(&core)
        .expect("an encoder")
        .validate(true)
        .encode()
        .expect("a component")
}

#[test]
fn prepare_returns_the_operations_the_component_declares() {
    let scratch = Scratch::new();
    let manifest = scratch.join("provider.toml");
    fs::write(
        &manifest,
        "name = \"guest\"\n\n[kind.file]\nimmutable = [\"path\"]\n",
    )
    .expect("a manifest");
    let component_path = scratch.join("provider.wasm");
    fs::write(&component_path, component()).expect("a component");

    let wasm = Wasm::load(&manifest, &component_path).expect("a loaded provider");
    assert_eq!(wasm.name().as_str(), "guest");

    let file = address("guest:file.note@localhost");
    let desired = [(String::from("path"), Value::Text(String::from("/tmp/note")))]
        .into_iter()
        .collect();
    let secret = BTreeSet::new();
    let observed = Observed::new();
    let facts = observed.facts_of(None);
    let change = Prepared {
        address: &file,
        kind: Sending::Create,
        roused: false,
        resent: false,
        desired: &desired,
        secret: &secret,
        recorded: None,
        facts,
    };

    let planned = wasm.prepare(&change).expect("a plan");
    assert_eq!(planned.operations.len(), 1, "one operation: {planned:?}");
    let Some(nerpa_proto::Operation::File { path, content, .. }) = planned.operations.first()
    else {
        panic!("expected a file operation: {planned:?}");
    };
    assert_eq!(path, "/tmp/nerpa-wasm-created");
    assert_eq!(content, b"made by the wasm provider\n");
}
