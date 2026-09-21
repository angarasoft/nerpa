//! Moving a version 4 state file into a version 5 directory.
//!
//! A directory cannot be renamed onto a file, and renaming in the other order
//! leaves a moment with no state at all — which today's loader reads as a first
//! run with a fresh key and nothing recorded. So the directory lives at a path of
//! its own, `<state>.d/`, and the version 4 file is kept beside it until somebody
//! says it is no longer wanted. RFC 0001 §5.13.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use nerpa_core::Value;
use nerpa_core::address::address;
use nerpa_core::provider::Attributes;
use nerpa_core::secret::{Key, Recorded};
use nerpa_core::state::{Record, State};
use nerpa_core::{shards, store};

static DIGEST_KEY: Key = Key::from_bytes([7; 32]);

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("nerpa-migration-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("a scratch directory");
        Self(path)
    }

    /// Where a version 4 file lives.
    fn file(&self) -> PathBuf {
        self.0.join("state.json")
    }

    /// And where the directory built from it goes.
    fn directory(&self) -> PathBuf {
        self.0.join("state.json.d")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn populated() -> State {
    let mut state = State::new();
    let mut motd = Attributes::new();
    motd.insert("path".to_owned(), Value::Text("/etc/motd".to_owned()));
    state.record(
        address("os:file.motd@web-01"),
        Record::new(Recorded::all(motd), BTreeSet::new()),
    );
    state.record(
        address("yandex:instance.db"),
        Record::new(Recorded::all(Attributes::new()), BTreeSet::new()),
    );
    state
}

#[test]
fn a_version_four_file_becomes_a_directory() {
    let scratch = Scratch::new("forward");
    let written = populated();
    store::save(&scratch.file(), &written, &DIGEST_KEY).expect("a version 4 file");

    shards::migrate(&scratch.file()).expect("it migrates");

    let (read, key) = shards::load(&scratch.directory()).expect("the directory holds it");
    assert_eq!(read, written, "everything it held came across");
    assert_eq!(
        key.to_hex(),
        DIGEST_KEY.to_hex(),
        "under the same key, or every secret would read as changed"
    );

    // The old file is kept where a downgrade can find it, and what is left at
    // the old path is something a version 4 binary refuses rather than reads as
    // a first run.
    assert!(
        scratch.0.join("state.json.v4").exists(),
        "the original is kept"
    );
    let refusal = store::load(&scratch.file()).expect_err("version 4 must not read it");
    assert!(
        refusal.to_string().contains("version"),
        "and it is refused by version, not by shape: {refusal}"
    );
}

#[test]
fn a_migration_that_stopped_before_it_finished_is_finished() {
    let scratch = Scratch::new("unfinished");
    let written = populated();
    store::save(&scratch.file(), &written, &DIGEST_KEY).expect("a version 4 file");

    // A directory with no `format` is a migration that did not get that far.
    fs::create_dir_all(scratch.directory()).unwrap();
    fs::write(scratch.directory().join("key"), "not even a key").unwrap();

    shards::migrate(&scratch.file()).expect("it starts again");

    let (read, _) = shards::load(&scratch.directory()).expect("the directory holds it");
    assert_eq!(read, written);
}

#[test]
fn a_version_four_file_of_another_digest_is_refused() {
    let scratch = Scratch::new("stranger");
    store::save(&scratch.file(), &populated(), &DIGEST_KEY).expect("a version 4 file");
    shards::migrate(&scratch.file()).expect("it migrates");

    // Somebody puts a different version 4 file back where the old one was.
    let mut other = populated();
    other.forget(&address("yandex:instance.db"));
    store::save(&scratch.file(), &other, &DIGEST_KEY).expect("another version 4 file");

    let refusal =
        shards::migrate(&scratch.file()).expect_err("it is not the file this was built from");
    assert!(
        refusal.to_string().contains("state.json"),
        "the refusal names it: {refusal}"
    );
}
