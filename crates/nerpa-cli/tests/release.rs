//! What `nerpa release` does to a state directory.
//!
//! The command exists for a person holding a machine nobody can describe, and
//! what it is worth is what it leaves behind on disk. So it is run as a person
//! runs it — the built binary, a real state directory — rather than by calling
//! the library it calls. RFC 0001 §5.6, §5.8.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::path::Path;
use std::process::{Command, Output};

use nerpa_core::address::address;
use nerpa_core::plan::ChangeKind;
use nerpa_core::provider::Attributes;
use nerpa_core::state::{Mark, Standing, State};
use nerpa_core::{Value, shards};

const MOTD: &str = "os:file.motd@web-01";

/// A state directory holding one mark for `MOTD`, standing where it is told.
fn holding(state: &Path, stands: Standing) {
    let (mut held, key, _lock) =
        shards::opened(state, shards::Taking::Writing).expect("a new state directory");
    let mut locator = Attributes::new();
    locator.insert("path".to_owned(), Value::Text("/etc/motd".to_owned()));
    held.mark(
        address(MOTD),
        Mark::new(ChangeKind::Update, locator, Attributes::new()).standing(stands),
    );
    shards::save(&shards::beside(state), &held, &key).expect("saved");
}

/// What state says afterwards, read as another run would read it.
fn afterwards(state: &Path) -> State {
    let (read, _key, _lock) = shards::opened(state, shards::Taking::Reading).expect("readable");
    read
}

fn nerpa(state: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_nerpa"))
        .args(arguments)
        // A person who looked at the machine is acting (`INV-TRUST-001`), and
        // a run without one does not start.
        .args(["--as", "tester@example.invalid"])
        .arg("--state")
        .arg(state)
        .output()
        .expect("the command runs")
}

/// A person who looked at the machine can clear one entry.
#[test]
fn an_entry_a_person_cleared_is_gone_from_the_state_directory() {
    let scratch = tempfile::tempdir().unwrap();
    let state = scratch.path().join("state.json");
    holding(&state, Standing::ForRemovalOnly);

    let said = nerpa(&state, &["release", "--entry", MOTD]);
    let out = String::from_utf8_lossy(&said.stdout).to_string();
    assert!(
        said.status.success(),
        "{out}{}",
        String::from_utf8_lossy(&said.stderr)
    );
    assert!(
        out.contains("the entry is cleared") && out.contains("not Nerpa's to remove"),
        "it says what it did and what that means: {out}"
    );
    assert!(
        afterwards(&state).marked(&address(MOTD)).is_none(),
        "and the state directory no longer remembers it"
    );
}

/// A change still in flight is not an entry, and the command says which step
/// comes first rather than doing it silently.
#[test]
fn a_change_in_flight_is_refused_and_left_alone() {
    let scratch = tempfile::tempdir().unwrap();
    let state = scratch.path().join("state.json");
    holding(&state, Standing::InFlight);

    let said = nerpa(&state, &["release", "--entry", MOTD]);
    assert!(!said.status.success(), "it refuses");
    let complaint = String::from_utf8_lossy(&said.stderr).to_string();
    assert!(
        complaint.contains("release its machine first"),
        "and names the smaller step: {complaint}"
    );
    assert!(
        afterwards(&state)
            .marked(&address(MOTD))
            .is_some_and(Mark::in_flight),
        "leaving what was there exactly as it was"
    );
}

/// Naming a machine and an entry at once is refused before anything is opened:
/// they are different things to say about different amounts of a run.
#[test]
fn a_machine_and_an_entry_are_not_asked_for_together() {
    let scratch = tempfile::tempdir().unwrap();
    let state = scratch.path().join("state.json");

    let said = nerpa(&state, &["release", "web-01", "--entry", MOTD]);
    assert!(!said.status.success(), "it refuses");
    assert!(
        !state.with_extension("json.d").exists(),
        "and nothing was opened to find that out"
    );
}
