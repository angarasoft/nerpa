//! The run log verifies from the first entry. `INV-LOG-001`, decision 0009.
//!
//! Run as a person runs it — the built binary — because what this is about is
//! a tampered journal being refused by the command that reads it, and a
//! library cannot show that.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::path::Path;
use std::process::{Command, Output};

/// Runs nerpa against one state file, as somebody.
fn nerpa(state: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_nerpa"))
        .args(arguments)
        .arg("--state")
        .arg(state)
        .output()
        .unwrap()
}

/// A configuration with one file, on no machine but this one.
fn configuration(root: &Path) -> std::path::PathBuf {
    let source = root.join("main.star");
    std::fs::write(&source, "node(\"localhost\")\n").unwrap();
    source
}

/// Appends a line to the log the way a run does, by running a plan.
fn plan(root: &Path, state: &Path) {
    let said = nerpa(
        state,
        &[
            "plan",
            "--as",
            "alice@example.com",
            "-f",
            configuration(root).to_str().unwrap(),
        ],
    );
    assert!(
        said.status.success(),
        "{}",
        String::from_utf8_lossy(&said.stderr)
    );
}

/// What the log holds, and where it lives.
fn log(state: &Path) -> (std::path::PathBuf, Vec<String>) {
    let mut named = state.as_os_str().to_owned();
    named.push(".journal");
    let where_to = std::path::PathBuf::from(named);
    let written = std::fs::read_to_string(&where_to).unwrap();
    let lines: Vec<String> = written.lines().map(str::to_owned).collect();
    (where_to, lines)
}

/// What is written verifies, and says how much of it there is.
#[test]
fn the_log_verifies_from_the_first_entry() {
    let scratch = tempfile::tempdir().unwrap();
    let root = scratch.path();
    let state = root.join("state.json");

    plan(root, &state);
    plan(root, &state);

    let said = nerpa(&state, &["journal", "verify", "--as", "alice@example.com"]);
    assert!(said.status.success(), "{said:?}");
    let out = String::from_utf8_lossy(&said.stdout);
    assert!(
        out.contains("verifies from the first entry: 2 entries"),
        "{out}"
    );
}

/// `INV-LOG-001` names its bypass: an entry rewritten while the rest of the log
/// is left exactly as it was. The chain breaks at the entry that moved, and the
/// refusal names it rather than saying only "broken".
#[test]
#[dacc_derive::doc_anchor(id = "a-rewritten-entry-is-found-and-named")]
fn a_rewritten_entry_is_found_and_named() {
    let scratch = tempfile::tempdir().unwrap();
    let root = scratch.path();
    let state = root.join("state.json");

    plan(root, &state);
    plan(root, &state);

    let (where_to, mut lines) = log(&state);
    assert_eq!(lines.len(), 2, "{lines:?}");
    // The ordinary tamper: one line, and no tooling. The hash field is left
    // untouched, because a forger who recomputes it is the case this entry
    // names as beyond a chain over itself.
    lines[0] = lines[0].replace("as alice@example.com", "as mallory@example.com");
    std::fs::write(&where_to, format!("{}\n", lines.join("\n"))).unwrap();

    let said = nerpa(&state, &["journal", "verify", "--as", "alice@example.com"]);
    assert!(!said.status.success(), "a rewritten entry must not verify");
    let problem = String::from_utf8_lossy(&said.stderr);
    assert!(
        problem.contains("entry 1") && problem.contains("does not chain"),
        "{problem}"
    );
}

/// The same bypass taken another way: an entry removed from the middle, so the
/// one after it no longer carries the entry before it.
#[test]
#[dacc_derive::doc_anchor(id = "a-removed-entry-is-found")]
fn a_removed_entry_is_found() {
    let scratch = tempfile::tempdir().unwrap();
    let root = scratch.path();
    let state = root.join("state.json");

    plan(root, &state);
    plan(root, &state);
    plan(root, &state);

    let (where_to, mut lines) = log(&state);
    assert_eq!(lines.len(), 3, "{lines:?}");
    lines.remove(1);
    std::fs::write(&where_to, format!("{}\n", lines.join("\n"))).unwrap();

    let said = nerpa(&state, &["journal", "verify", "--as", "alice@example.com"]);
    assert!(!said.status.success(), "a log with a gap must not verify");
    let problem = String::from_utf8_lossy(&said.stderr);
    assert!(
        problem.contains("entry 2") && problem.contains("does not chain"),
        "{problem}"
    );
}

/// What the chain cannot see is said rather than claimed away: truncating the
/// tail leaves a chain that still verifies, so the count is part of the answer
/// and the log is not proof of its own completeness.
#[test]
fn a_truncated_log_still_verifies_and_the_count_says_so() {
    let scratch = tempfile::tempdir().unwrap();
    let root = scratch.path();
    let state = root.join("state.json");

    plan(root, &state);
    plan(root, &state);
    plan(root, &state);

    let (where_to, mut lines) = log(&state);
    lines.pop();
    std::fs::write(&where_to, format!("{}\n", lines.join("\n"))).unwrap();

    let said = nerpa(&state, &["journal", "verify", "--as", "alice@example.com"]);
    assert!(said.status.success(), "{said:?}");
    let out = String::from_utf8_lossy(&said.stdout);
    assert!(
        out.contains("2 entries"),
        "the count is what a person compares against what they expect: {out}"
    );
}

/// A journal of another shape is not read as this one and not appended to: a
/// record nothing can walk from the first entry is not one to extend.
#[test]
fn a_journal_of_another_shape_is_refused_not_extended() {
    let scratch = tempfile::tempdir().unwrap();
    let root = scratch.path();
    let state = root.join("state.json");
    configuration(root);

    let mut named = state.as_os_str().to_owned();
    named.push(".journal");
    std::fs::write(
        std::path::PathBuf::from(named),
        "1700000000 plan 00 as alice over run until further notice\n",
    )
    .unwrap();

    let said = nerpa(
        &state,
        &[
            "plan",
            "--as",
            "alice@example.com",
            "-f",
            configuration(root).to_str().unwrap(),
        ],
    );
    // The run itself still happens — a journal that cannot be written says so
    // out loud rather than failing a run that already happened — but nothing
    // is appended to a log that cannot be verified.
    assert!(said.status.success(), "{said:?}");
    let problem = String::from_utf8_lossy(&said.stderr);
    assert!(
        problem.contains("not a hash chain"),
        "the refusal names the reason: {problem}"
    );
    let (_, lines) = log(&state);
    assert_eq!(lines.len(), 1, "and the log is left as it was: {lines:?}");
}
