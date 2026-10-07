//! A WASM provider wired through the whole run creates a resource on a real
//! machine: read, plan, apply, and the file is there.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::fs;
use std::path::PathBuf;

use nerpa_core::address::address;
use nerpa_core::provider::{Machines, Providers};
use nerpa_core::run::{apply, observe};
use nerpa_core::secret::{Key, Remembered, Secrets};
use nerpa_core::state::State;
use nerpa_core::{Actor, Grant, Graph, Observed, Plan, Resource};
use nerpa_provider_wasm::Wasm;
use nerpa_transport::Here;

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

/// A directory that removes itself.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("nerpa-wasm-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("a scratch directory");
        Self(path)
    }

    fn join(&self, name: &str) -> String {
        self.0.join(name).display().to_string()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A fixed key and a store holding nothing, so the digests are reproducible.
static DIGEST_KEY: Key = Key::from_bytes([7; 32]);
static NO_STORE: Remembered = Remembered::EMPTY;

fn secrets() -> Secrets<'static> {
    Secrets::new(&DIGEST_KEY, &NO_STORE)
}

/// Every machine is this one, reached by the executor in this process.
fn machines() -> Machines {
    Machines::new().everywhere(Box::new(Here))
}

/// A run that does not care where state goes.
#[allow(
    clippy::unnecessary_wraps,
    reason = "the shape belongs to `apply`, not to this function"
)]
fn kept(_: &State) -> Result<(), String> {
    Ok(())
}

/// Plans the change, attributed to a tester.
fn plan(
    graph: &Graph,
    observed: &Observed,
    state: &State,
    schema: &nerpa_core::Schema,
) -> Result<Plan, nerpa_core::planner::PlanError> {
    nerpa_core::planner::plan(graph, observed, state, schema, secrets()).map(|planned| {
        planned.attributed_to(Actor::new("tester", Grant::default(), None).expect("an actor"))
    })
}

#[test]
fn a_wasm_provider_creates_the_file_the_run_declares() {
    let scratch = Scratch::new("apply");
    let path = scratch.join("note");

    let manifest = scratch.0.join("provider.toml");
    fs::write(
        &manifest,
        "name = \"guest\"\n\n[kind.file]\nimmutable = [\"path\"]\nidentity = { attribute = \"path\", namespace = \"path\" }\n",
    )
    .expect("a manifest");
    let component_path = scratch.0.join("provider.wasm");
    fs::write(&component_path, component()).expect("a component");
    let wasm = Wasm::load(&manifest, &component_path).expect("a loaded provider");

    let providers = Providers::new().with(Box::new(wasm));
    let graph = {
        let mut graph = Graph::new();
        graph.preexisting(nerpa_core::Target::new("localhost").expect("a node"));
        graph
            .insert(Resource::new(address("guest:file.note@localhost")).with("path", path.clone()))
            .expect("a distinct address");
        graph
    };
    let mut state = State::new();

    let observed = observe(&graph, &providers, &machines(), &state).expect("readable");
    let planned = plan(&graph, &observed, &state, &providers.schema()).expect("plannable");
    assert_eq!(
        planned.blast_radius().creates(),
        1,
        "the run proposes one creation: {planned:?}"
    );

    apply(
        &planned,
        &providers,
        &machines(),
        &observed,
        &mut state,
        secrets(),
        &mut kept,
    )
    .expect("applies");

    assert_eq!(
        fs::read_to_string(&path).ok().as_deref(),
        Some("made by the wasm provider\n"),
        "the file the component declared is created"
    );
}
