//! State as a directory: a file per machine, and nothing else believed.
//!
//! Format 4 was one file for a whole fleet, so every machine's record waited on
//! every other machine's write. Format 5 is a directory with a shard per machine
//! (RFC 0001 §5.6), which is what lets one machine's outcome reach the disk
//! without the fleet's.
//!
//! This is the layout alone: it holds what format 4 holds, and the marks,
//! entries and migration that the same section describes come after it.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use nerpa_core::Value;
use nerpa_core::address::address;
use nerpa_core::provider::Attributes;
use nerpa_core::secret::{Key, Recorded};
use nerpa_core::shards;
use nerpa_core::state::{Record, State};

/// A fixed key, so that what one test writes another reads back.
static DIGEST_KEY: Key = Key::from_bytes([7; 32]);

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("nerpa-shards-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("a scratch directory");
        Self(path)
    }

    /// Where the directory goes, which is a path of its own and never the
    /// version 4 file's.
    fn directory(&self) -> PathBuf {
        self.0.join("state.json.d")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Two machines and one resource that lives on neither.
fn populated() -> State {
    let mut state = State::new();
    let mut motd = Attributes::new();
    motd.insert("path".to_owned(), Value::Text("/etc/motd".to_owned()));
    state.record(
        address("os:file.motd@web-01"),
        Record::new(Recorded::all(motd.clone()), BTreeSet::new()),
    );
    state.record(
        address("os:file.motd@web-02"),
        Record::new(Recorded::all(motd), BTreeSet::new()),
    );
    state.record(
        address("yandex:instance.db"),
        Record::new(Recorded::all(Attributes::new()), BTreeSet::new()),
    );
    state
}

#[test]
fn state_survives_a_trip_through_the_directory() {
    let scratch = Scratch::new("round-trip");
    let written = populated();

    shards::save(&scratch.directory(), &written, &DIGEST_KEY).expect("it writes");
    let (read, key) = shards::load(&scratch.directory()).expect("it reads");

    assert_eq!(read, written);
    assert_eq!(key.to_hex(), DIGEST_KEY.to_hex());
}

#[test]
fn each_machine_has_a_shard_of_its_own() {
    let scratch = Scratch::new("per-machine");
    shards::save(&scratch.directory(), &populated(), &DIGEST_KEY).expect("it writes");

    let mut names: Vec<String> = fs::read_dir(scratch.directory())
        .expect("the directory is there")
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();

    assert!(names.contains(&"key".to_owned()), "{names:?}");
    assert!(names.contains(&"format".to_owned()), "{names:?}");
    assert!(
        names.contains(&"control".to_owned()),
        "what lives on no machine has a shard of its own: {names:?}"
    );
    let machines: Vec<&String> = names
        .iter()
        .filter(|name| name.len() == 64 && name.chars().all(|ch| ch.is_ascii_hexdigit()))
        .collect();
    assert_eq!(
        machines.len(),
        2,
        "one shard for each machine, named at a fixed length whatever the machine is: {names:?}"
    );
}

#[test]
fn a_name_nobody_defined_refuses_the_run() {
    let scratch = Scratch::new("stranger");
    shards::save(&scratch.directory(), &populated(), &DIGEST_KEY).expect("it writes");
    fs::write(scratch.directory().join("notes.txt"), "mine").unwrap();

    let refusal = shards::load(&scratch.directory()).expect_err("a name nobody defined");
    assert!(
        refusal.to_string().contains("notes.txt"),
        "the refusal names it: {refusal}"
    );
}

#[test]
fn a_first_run_has_no_state_and_that_is_not_an_error() {
    let scratch = Scratch::new("first");
    let (state, key) = shards::load(&scratch.directory()).expect("a first run");

    assert!(state.is_empty());
    assert_eq!(key.to_hex().len(), 64, "and it has a key of its own");
}
