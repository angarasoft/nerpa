//! Takes a recording of the converge scenario from a live Yandex Cloud.
//!
//! The ignored test that writes `tests/common/recordings/converge.json`: run
//! it under `YC_FOLDER_ID` and `YC_CREDENTIALS_FILE` with
//! `cargo test -p nerpa-provider-yandex --test record -- --ignored`. The
//! tunnel fans the calls out to the live service hosts itself. The scenario
//! makes real resources in the named folder — exactly the five the converge
//! tests declare — so point it at a throwaway folder and delete what it made
//! afterwards.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

mod common;

use std::path::Path;

use nerpa_core::provider::Machines;
use nerpa_core::state::State;
use nerpa_provider_yandex::credentials::Credentials;

use common::recorded::Recorder;
use common::{Declared, converge, declared_recordable, providers};

#[test]
#[ignore = "needs a live cloud: set YC_FOLDER_ID and YC_CREDENTIALS_FILE, then run with --ignored"]
fn take_a_recording_of_the_converge_scenario() {
    let folder = std::env::var("YC_FOLDER_ID").expect("YC_FOLDER_ID");
    let credentials_file = std::env::var("YC_CREDENTIALS_FILE").expect("YC_CREDENTIALS_FILE");
    let credentials = Credentials::from_json(
        &std::fs::read_to_string(&credentials_file).expect("a credential file"),
    )
    .expect("a usable credential");
    let (recorder, address) = Recorder::start(&credentials);
    let providers = providers(&address);
    let machines = Machines::new();
    let mut state = State::new();
    let graph = declared_recordable(&folder, Declared::default());
    // Two runs, exactly as the replay test drives them: the first makes the
    // world, the second proves the plan is empty.
    converge(&graph, &providers, &machines, &mut state);
    converge(&graph, &providers, &machines, &mut state);
    let recording = recorder.into_recording(folder);
    recording
        .save(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/common/recordings/converge.json"));
}
