//! Who acts in a run, at the command line. `INV-TRUST-001`, RFC 0001 §5.9.
//!
//! Run as a person runs it — the built binary — because the refusal this is
//! about is a run not starting, and a library cannot show that.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::process::Command;

/// `INV-TRUST-001` names its bypass: a run that starts without an actor
/// identity. The flag is required, and the refusal names it.
#[test]
#[dacc_derive::doc_anchor(id = "a-run-without-an-actor-is-refused-naming-the-flag")]
fn a_run_without_an_actor_is_refused_naming_the_flag() {
    let scratch = tempfile::tempdir().unwrap();
    let said = Command::new(env!("CARGO_BIN_EXE_nerpa"))
        .arg("plan")
        .arg("--state")
        .arg(scratch.path().join("state.json"))
        .output()
        .unwrap();
    assert!(!said.status.success(), "a run without --as must not start");
    let problem = String::from_utf8_lossy(&said.stderr);
    assert!(problem.contains("--as"), "{problem}");
}

/// The journal beside state says who acted: the durable half of the identity,
/// for a person reading afterwards (RFC 0001 §5.9).
#[test]
fn a_run_journal_says_who_acted() {
    let scratch = tempfile::tempdir().unwrap();
    let root = scratch.path();
    std::fs::write(root.join("main.star"), "node(\"localhost\")\n").unwrap();
    let state = root.join("state.json");

    let said = Command::new(env!("CARGO_BIN_EXE_nerpa"))
        .args(["plan", "--as", "alice@example.com"])
        .arg("--state")
        .arg(&state)
        .arg("-f")
        .arg(root.join("main.star"))
        .output()
        .unwrap();
    assert!(
        said.status.success(),
        "{}",
        String::from_utf8_lossy(&said.stderr)
    );

    let journal = std::fs::read_to_string(root.join("state.json.journal")).unwrap();
    assert!(journal.contains("plan "), "{journal}");
    assert!(
        journal.contains("as alice@example.com over run"),
        "{journal}"
    );
}

/// `--as-expiry` is a boundary, not a label (review v0013's minor-1): an
/// identity whose expiry has passed does not run, and the refusal names it.
#[test]
fn an_identity_whose_expiry_passed_does_not_run() {
    let scratch = tempfile::tempdir().unwrap();
    let said = Command::new(env!("CARGO_BIN_EXE_nerpa"))
        .args([
            "plan",
            "--as",
            "alice@example.com",
            "--as-expiry",
            "2000-01-01T00:00:00Z",
        ])
        .arg("--state")
        .arg(scratch.path().join("state.json"))
        .output()
        .unwrap();
    assert!(
        !said.status.success(),
        "an identity whose expiry passed must not run"
    );
    let problem = String::from_utf8_lossy(&said.stderr);
    assert!(
        problem.contains("expiry") && problem.contains("2000-01-01T00:00:00Z"),
        "{problem}"
    );
}

/// An expiry that is not an RFC3339 time is refused naming the format and the
/// value, rather than carried as a string that means nothing.
#[test]
fn an_expiry_that_is_not_a_time_is_refused_by_name() {
    let scratch = tempfile::tempdir().unwrap();
    let said = Command::new(env!("CARGO_BIN_EXE_nerpa"))
        .args([
            "plan",
            "--as",
            "alice@example.com",
            "--as-expiry",
            "yesterday",
        ])
        .arg("--state")
        .arg(scratch.path().join("state.json"))
        .output()
        .unwrap();
    assert!(
        !said.status.success(),
        "an expiry that is not a time must not run"
    );
    let problem = String::from_utf8_lossy(&said.stderr);
    assert!(
        problem.contains("RFC3339") && problem.contains("yesterday"),
        "{problem}"
    );
}

/// The future expiry is what the flag is for: the run proceeds.
#[test]
fn an_identity_with_a_future_expiry_runs() {
    let scratch = tempfile::tempdir().unwrap();
    let root = scratch.path();
    std::fs::write(root.join("main.star"), "node(\"localhost\")\n").unwrap();
    let said = Command::new(env!("CARGO_BIN_EXE_nerpa"))
        .args([
            "plan",
            "--as",
            "alice@example.com",
            "--as-expiry",
            "2999-06-01T00:00:00Z",
        ])
        .arg("--state")
        .arg(root.join("state.json"))
        .arg("-f")
        .arg(root.join("main.star"))
        .output()
        .unwrap();
    assert!(
        said.status.success(),
        "{}",
        String::from_utf8_lossy(&said.stderr)
    );
}
