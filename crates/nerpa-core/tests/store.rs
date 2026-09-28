//! State on disk: what it round-trips, and what it refuses.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use nerpa_core::Value;
use nerpa_core::address::address;
use nerpa_core::provider::Attributes;
use nerpa_core::secret::{Key, Recorded};
use nerpa_core::state::{Record, State, Written};
use nerpa_core::store::{self, Lock, StoreError};

/// A fixed key, so that a file written by one test reads back in another and
/// the bytes on disk are the same every run.
static DIGEST_KEY: Key = Key::from_bytes([7; 32]);

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
        Record::new(Recorded::all(attributes), depends_on),
    );
    state.record(
        address("yandex:instance.db"),
        Record::new(Recorded::all(Attributes::new()), BTreeSet::new()),
    );
    state
}

#[test]
fn state_survives_a_trip_through_the_file() {
    let scratch = Scratch::new("roundtrip");
    let written = populated();

    store::save(&scratch.file(), &written, &DIGEST_KEY).expect("writes");
    let read = store::load(&scratch.file())
        .map(|(state, _)| state)
        .expect("reads");

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
    store::save(&scratch.file(), &populated(), &DIGEST_KEY).expect("writes");

    let text = fs::read_to_string(scratch.file()).expect("written");
    assert!(
        text.contains(&format!("\"version\": {}", store::FORMAT_VERSION)),
        "{text}"
    );
    assert!(
        text.contains("\"os:file.motd@web-01\""),
        "addresses are text: {text}"
    );
    assert!(text.contains("\"/etc/motd\""), "{text}");
    assert!(
        text.contains("\"digest_key\""),
        "the key its digests were taken under travels with them: {text}"
    );
    assert!(text.contains('\n'), "and it is not one long line");
}

#[test]
fn a_first_run_has_no_state_and_that_is_not_an_error() {
    let scratch = Scratch::new("first");
    let state = store::load(&scratch.file())
        .map(|(state, _)| state)
        .expect("an absent file is an empty state");
    assert!(state.is_empty());
}

#[test]
#[dacc_derive::doc_anchor(id = "a-file-from-a-version-this-one-does-not-understand-is-refused")]
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
        store::load(&scratch.file()).map(|(state, _)| state),
        Err(StoreError::Format { .. })
    ));
}

#[test]
fn an_address_the_model_refuses_is_refused_on_the_way_in_too() {
    let scratch = Scratch::new("address");
    fs::write(
        scratch.file(),
        r#"{"version": 4, "digest_key": "0707070707070707070707070707070707070707070707070707070707070707", "resources": {"not-an-address": {"attributes": {}, "depends_on": [], "written": 0}}}"#,
    )
    .expect("written");

    let refusal = store::load(&scratch.file()).expect_err("state is not a way around the grammar");
    assert!(refusal.to_string().contains("not-an-address"), "{refusal}");
}

/// A half-written state file claims things that are not true, which is worse
/// than no state file at all.
#[test]
#[dacc_derive::doc_anchor(id = "writing-replaces-the-previous-file-only-once-it-is-whole")]
fn writing_replaces_the_previous_file_only_once_it_is_whole() {
    let scratch = Scratch::new("atomic");
    store::save(&scratch.file(), &populated(), &DIGEST_KEY).expect("writes");

    let mut smaller = State::new();
    smaller.record(
        address("yandex:instance.db"),
        Record::new(Recorded::all(Attributes::new()), BTreeSet::new()),
    );
    store::save(&scratch.file(), &smaller, &DIGEST_KEY).expect("writes again");

    assert_eq!(
        store::load(&scratch.file())
            .map(|(state, _)| state)
            .expect("reads")
            .len(),
        1
    );
    assert!(
        !scratch.0.join("state.json.new").exists(),
        "no leftovers beside it"
    );
}

#[test]
#[dacc_derive::doc_anchor(id = "two-runs-cannot-hold-the-state-at-once")]
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
            Recorded::all(Attributes::new()),
            BTreeSet::new(),
            Written::at(1_700_000_000),
        ),
    );

    store::save(&scratch.file(), &state, &DIGEST_KEY).expect("writes");
    let read = store::load(&scratch.file())
        .map(|(state, _)| state)
        .expect("reads");

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

/// A lock file left behind by a run that was killed is not a lock.
///
/// It was: the file's existence was the lock, so a run that died — a Ctrl-C at
/// the wrong moment, a machine rebooted mid-run — left a file that refused every
/// later run until somebody deleted it by hand. Deleting it by hand is the one
/// thing a lock exists to stop people doing carelessly. Held by the kernel
/// instead, it is released when the process ends, however it ends. RFC 0001
/// §5.8.
#[test]
#[dacc_derive::doc_anchor(id = "a-lock-file-left-behind-by-a-dead-run-is-not-a-lock")]
fn a_lock_file_left_behind_by_a_dead_run_is_not_a_lock() {
    let scratch = Scratch::new("stale");
    let mut left = scratch.file().into_os_string();
    left.push(".lock");
    fs::write(&left, "pid 1234 since 1700000000").unwrap();

    let taken = Lock::take(&scratch.file());
    assert!(
        taken.is_ok(),
        "nobody holds it, whatever the file says: {taken:?}"
    );
}

/// Planning reads and writes nothing, so several plans may hold the lock at
/// once; a run that writes may not start while any of them is reading.
///
/// A lock file's existence cannot say this: it is held or it is not. RFC 0001
/// §5.8, where `plan` takes it shared so that it never reads a live run's marks
/// as though that run had ended.
#[test]
#[dacc_derive::doc_anchor(id = "two-plans-may-read-at-once-and-a-run-that-writes-may-not")]
fn two_plans_may_read_at_once_and_a_run_that_writes_may_not() {
    let scratch = Scratch::new("shared");
    let reading = Lock::shared(&scratch.file()).expect("nobody holds it");
    let alongside = Lock::shared(&scratch.file()).expect("one reader does not keep another out");

    let refused = Lock::take(&scratch.file());
    assert!(
        matches!(refused, Err(StoreError::Locked { .. })),
        "a run that writes waits for the readers: {refused:?}"
    );

    drop(reading);
    drop(alongside);
    Lock::take(&scratch.file()).expect("and once they are done it may write");
}
