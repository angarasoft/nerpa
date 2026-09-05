//! State on disk: what it round-trips, and what it refuses.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use nerpa_core::Value;
use nerpa_core::address::address;
use nerpa_core::provider::Attributes;
use nerpa_core::state::{Record, State, Written};
use nerpa_core::store::{self, Lock, StoreError};

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("nerpa-store-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("a scratch directory");
        Self(path)
    }

    fn file(&self) -> PathBuf {
        self.0.join("state.json")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn populated() -> State {
    let mut state = State::new();
    let mut attributes = Attributes::new();
    attributes.insert("path".to_owned(), Value::Text("/etc/motd".to_owned()));
    attributes.insert("cores".to_owned(), Value::Integer(8));
    attributes.insert("enabled".to_owned(), Value::Boolean(true));
    attributes.insert(
        "tags".to_owned(),
        Value::List(vec![Value::Text("a".to_owned()), Value::Integer(2)]),
    );

    let depends_on: BTreeSet<_> = [address("yandex:instance.db")].into_iter().collect();
    state.record(
        address("os:file.motd@web-01"),
        Record::new(attributes, depends_on),
    );
    state.record(
        address("yandex:instance.db"),
        Record::new(Attributes::new(), BTreeSet::new()),
    );
    state
}

#[test]
fn state_survives_a_trip_through_the_file() {
    let scratch = Scratch::new("roundtrip");
    let written = populated();

    store::save(&scratch.file(), &written).expect("writes");
    let read = store::load(&scratch.file()).expect("reads");

    assert_eq!(
        read, written,
        "everything came back, including the recorded edges"
    );
    assert_eq!(read.len(), 2);
    assert_eq!(
        read.get(&address("os:file.motd@web-01"))
            .expect("recorded")
            .depends_on()
            .len(),
        1
    );
}

/// A person opens this file with `jq` at some point, usually a bad one.
#[test]
fn the_file_is_readable_by_a_person() {
    let scratch = Scratch::new("readable");
    store::save(&scratch.file(), &populated()).expect("writes");

    let text = fs::read_to_string(scratch.file()).expect("written");
    assert!(text.contains("\"version\": 2"), "{text}");
    assert!(
        text.contains("\"os:file.motd@web-01\""),
        "addresses are text: {text}"
    );
    assert!(text.contains("\"/etc/motd\""), "{text}");
    assert!(text.contains('\n'), "and it is not one long line");
}

#[test]
fn a_first_run_has_no_state_and_that_is_not_an_error() {
    let scratch = Scratch::new("first");
    let state = store::load(&scratch.file()).expect("an absent file is an empty state");
    assert!(state.is_empty());
}

#[test]
fn a_file_from_a_version_this_one_does_not_understand_is_refused() {
    let scratch = Scratch::new("version");
    fs::write(scratch.file(), r#"{"version": 99, "resources": {}}"#).expect("written");

    let refusal = store::load(&scratch.file()).expect_err("a later format is not guessed at");
    assert!(matches!(refusal, StoreError::Format { .. }));
    assert!(refusal.to_string().contains("version 99"), "{refusal}");
}

#[test]
fn a_file_that_is_not_state_is_refused_rather_than_read_as_empty() {
    let scratch = Scratch::new("garbage");
    fs::write(scratch.file(), "this is not json").expect("written");

    assert!(matches!(
        store::load(&scratch.file()),
        Err(StoreError::Format { .. })
    ));
}

#[test]
fn an_address_the_model_refuses_is_refused_on_the_way_in_too() {
    let scratch = Scratch::new("address");
    fs::write(
        scratch.file(),
        r#"{"version": 2, "resources": {"not-an-address": {"attributes": {}, "depends_on": [], "written": 0}}}"#,
    )
    .expect("written");

    let refusal = store::load(&scratch.file()).expect_err("state is not a way around the grammar");
    assert!(refusal.to_string().contains("not-an-address"), "{refusal}");
}

/// A half-written state file claims things that are not true, which is worse
/// than no state file at all.
#[test]
fn writing_replaces_the_previous_file_only_once_it_is_whole() {
    let scratch = Scratch::new("atomic");
    store::save(&scratch.file(), &populated()).expect("writes");

    let mut smaller = State::new();
    smaller.record(
        address("yandex:instance.db"),
        Record::new(Attributes::new(), BTreeSet::new()),
    );
    store::save(&scratch.file(), &smaller).expect("writes again");

    assert_eq!(store::load(&scratch.file()).expect("reads").len(), 1);
    assert!(
        !scratch.0.join("state.json.new").exists(),
        "no leftovers beside it"
    );
}

#[test]
fn two_runs_cannot_hold_the_state_at_once() {
    let scratch = Scratch::new("lock");
    let held = Lock::take(&scratch.file()).expect("nobody else has it");

    let refused = Lock::take(&scratch.file());
    assert!(
        matches!(refused, Err(StoreError::Locked { .. })),
        "{refused:?}"
    );

    drop(held);
    Lock::take(&scratch.file()).expect("released on the way out");
}

/// Not being there is one thing; not being readable is another. Treating every
/// failure as "no state yet" would propose creating a whole environment again.
#[test]
fn a_state_file_that_cannot_be_read_is_not_the_same_as_an_absent_one() {
    let scratch = Scratch::new("unreadable");
    fs::create_dir_all(scratch.file()).expect("a directory where the file should be");

    let refusal = store::load(&scratch.file()).expect_err("this is not an empty state");
    assert!(matches!(refusal, StoreError::File { .. }), "{refusal:?}");
}

/// A lock that could not be created is not the same as a lock somebody holds.
#[test]
fn a_lock_that_cannot_be_created_is_not_reported_as_held() {
    let scratch = Scratch::new("nolockdir");
    let nowhere = scratch.0.join("missing").join("state.json");

    let refusal = Lock::take(&nowhere).expect_err("there is no directory to put it in");
    assert!(matches!(refusal, StoreError::File { .. }), "{refusal:?}");
}

/// The age of what is known about a machine survives the trip, because that is
/// the only thing it exists for: saying how stale a plan's silence is.
#[test]
fn when_a_record_was_written_comes_back_too() {
    let scratch = Scratch::new("written");
    let mut state = State::new();
    state.record(
        address("os:file.motd@web-01"),
        Record::written_at(
            Attributes::new(),
            BTreeSet::new(),
            Written::at(1_700_000_000),
        ),
    );

    store::save(&scratch.file(), &state).expect("writes");
    let read = store::load(&scratch.file()).expect("reads");

    let recorded = read.get(&address("os:file.motd@web-01")).expect("recorded");
    assert_eq!(recorded.written(), Written::at(1_700_000_000));
    assert_eq!(
        recorded.written().age(Written::at(1_700_003_600)).as_secs(),
        3600
    );
}

/// A clock that moved backwards is not a negative age.
#[test]
fn a_record_from_the_future_is_reported_as_no_age_at_all() {
    let written = Written::at(2_000_000_000);
    assert_eq!(written.age(Written::at(1_000_000_000)).as_secs(), 0);
}
