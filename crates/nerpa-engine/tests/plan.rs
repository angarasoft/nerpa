//! The engine plans and applies as a library, over the opened machines.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::fs;
use std::path::PathBuf;

use nerpa_config::fleet::Fleet;
use nerpa_core::provider::Machines;
use nerpa_core::secret::{Key, Remembered, Secrets};
use nerpa_core::state::State;
use nerpa_core::{Actor, Grant};
use nerpa_engine::Engine;
use nerpa_transport::Here;

/// A fixed key and a store holding nothing: no test here declares a secret, and
/// a fixed key keeps digests reproducible.
static DIGEST_KEY: Key = Key::from_bytes([7; 32]);
static NO_STORE: Remembered = Remembered::EMPTY;

fn secrets() -> Secrets<'static> {
    Secrets::new(&DIGEST_KEY, &NO_STORE)
}

/// The engine is asked who is running, and a test is a run of a kind.
fn actor() -> Actor {
    Actor::new("tester", Grant::default(), None).expect("an actor")
}

/// Every machine is this one, reached by the executor in this process.
fn machines() -> Machines {
    Machines::new().everywhere(Box::new(Here))
}

/// A run that does not care where state goes, only what ends up in it.
#[allow(
    clippy::unnecessary_wraps,
    reason = "the shape belongs to `apply`, not to this function"
)]
fn kept(_: &State) -> Result<(), String> {
    Ok(())
}

/// A directory that removes itself, so a failing test cannot leave litter behind.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("nerpa-{name}-{}", std::process::id()));
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

/// The whole loop through the library, not the command: assemble, plan, apply,
/// plan again. The last plan must be empty, which is what a converged world is.
#[test]
fn the_engine_plans_and_applies_over_the_opened_machines() {
    let scratch = Scratch::new("engine-converge");
    let file = scratch.join("note");
    let config = format!(
        "node(\"localhost\")\nresource(\"os:file.note@localhost\", {{\n    \"path\": {file:?},\n    \"content\": \"one\\n\",\n}})\n"
    );

    let fleet = Fleet::empty();
    let engine = Engine::assemble("main.star", &config, &scratch.0, &fleet).expect("an engine");
    let machines = machines();
    let secrets = secrets();

    let mut state = State::new();
    let observed = engine.observe(&machines, &state).expect("a reading");
    let plan = engine
        .plan(&observed, &state, secrets, actor())
        .expect("a plan");
    assert!(
        !plan.is_empty(),
        "a declared file that is absent is a change"
    );

    let mut written = kept;
    let applied = engine
        .apply(
            &plan,
            &machines,
            &observed,
            &mut state,
            secrets,
            &mut written,
        )
        .expect("an apply");
    assert_eq!(applied.changed(), 1, "one file is made");

    let observed = engine.observe(&machines, &state).expect("a reading");
    let plan = engine
        .plan(&observed, &state, secrets, actor())
        .expect("a plan");
    assert!(
        plan.is_empty(),
        "the applied world plans to nothing: {plan:?}"
    );
}
