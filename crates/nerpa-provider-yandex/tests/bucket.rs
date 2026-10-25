//! The bucket kind: a declared `yandex:bucket` converges against the fake
//! cloud, a recording of the live exchanges is taken and replayed, and a
//! bucket the world holds before the declaration is found rather than made.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

mod common;

use std::path::Path;

use nerpa_core::provider::Machines;
use nerpa_core::state::State;
use nerpa_provider_yandex::credentials::Credentials;

use common::recorded::{Recorder, Recording, Replay};
use common::{FakeYandex, converge, declared_bucket, providers};

/// The folder everything in these tests lives in.
const FOLDER: &str = "b1g-folder";

/// The bucket a recording makes. A bucket's name is unique across the whole
/// platform, so the fixture owns one of its own; the replay declares the
/// same name, which is what makes its requests match the recording.
const BUCKET: &str = "nerpa-fixture-bucket";

#[test]
fn a_declared_bucket_converges() {
    let (fake, address) = FakeYandex::start();
    let providers = providers(&address);
    let machines = Machines::new();
    let mut state = State::new();
    let graph = declared_bucket(FOLDER, "data");

    let first = converge(&graph, &providers, &machines, &mut state);
    assert_eq!(
        first.blast_radius().creates(),
        1,
        "one create per declared bucket"
    );
    let ledger = fake.ledger();
    assert!(
        ledger.contains(&"create-bucket data".to_owned()),
        "the bucket was made: {ledger:?}"
    );

    let second = converge(&graph, &providers, &machines, &mut state);
    assert!(second.is_empty(), "the world matches the declaration now");
}

#[test]
#[ignore = "needs a live cloud: set YC_FOLDER_ID and YC_CREDENTIALS_FILE, then run with --ignored"]
fn take_a_recording_of_the_bucket_scenario() {
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
    let graph = declared_bucket(&folder, BUCKET);
    // Two runs, exactly as the replay test drives them: the first makes the
    // bucket, the second proves the plan is empty. The bucket is deleted
    // from the cloud afterwards, so the recording replays a world that no
    // longer exists there.
    converge(&graph, &providers, &machines, &mut state);
    converge(&graph, &providers, &machines, &mut state);
    let recording = recorder.into_recording(folder);
    recording
        .save(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/common/recordings/bucket.json"));
}

#[test]
fn the_provider_converges_against_recorded_bucket_exchanges() {
    let recording = Recording::load(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/common/recordings/bucket.json"),
    );
    let folder = recording.folder.clone();
    let (_, address) = Replay::start(recording);
    let providers = providers(&address);
    let machines = Machines::new();
    let mut state = State::new();
    let graph = declared_bucket(&folder, BUCKET);

    let first = converge(&graph, &providers, &machines, &mut state);
    assert_eq!(
        first.blast_radius().creates(),
        1,
        "one create per declared bucket"
    );

    let second = converge(&graph, &providers, &machines, &mut state);
    assert!(
        second.is_empty(),
        "the recorded world matches the declaration"
    );
}
