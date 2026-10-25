//! The provider converges against a recording of real Yandex exchanges:
//! every request must match a recorded one byte for byte, and the second
//! plan must be empty — the round trip a live cloud once made, replayed
//! without one.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

mod common;

use std::path::Path;

use nerpa_core::provider::Machines;
use nerpa_core::state::State;

use common::recorded::{Recording, Replay};
use common::{Declared, converge, declared_recordable, providers};

#[test]
fn the_provider_converges_against_recorded_exchanges() {
    let recording = Recording::load(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/common/recordings/converge.json"),
    );
    let folder = recording.folder.clone();
    let (_, address) = Replay::start(recording);
    let providers = providers(&address);
    let machines = Machines::new();
    let mut state = State::new();
    let graph = declared_recordable(&folder, Declared::default());

    let first = converge(&graph, &providers, &machines, &mut state);
    assert_eq!(
        first.blast_radius().creates(),
        1,
        "one create per declared resource"
    );

    let second = converge(&graph, &providers, &machines, &mut state);
    assert!(
        second.is_empty(),
        "the recorded world matches the declaration"
    );
}
