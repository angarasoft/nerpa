//! Repository checks, as a program rather than a pile of shell.
//!
//! The rule this exists to serve: tooling leads the code. A check that arrives
//! after the thing it checks is always retrofitted onto whatever grew in its
//! absence, and it always has to make excuses. So the gate is here from the
//! start, small, and it grows as the code does — not the other way round.
//!
//! # What it does
//!
//! ```text
//!   cargo xtask ci          everything below, in the order CI runs it
//!
//!   cargo xtask fmt         formatting
//!   cargo xtask lint        clippy, then this repository's own rules
//!   cargo xtask test        the test suites, with and without contracts
//!   cargo xtask invariants  doc/invariant/ against the code, both ways
//!   cargo xtask docs        decision records, then rustdoc with warnings denied
//!   cargo xtask deny        dependency licences and advisories
//!
//!   cargo xtask mutants     mutation testing — minutes, so not on the commit path
//!   cargo xtask budget      what a plan costs — a release build, so likewise
//!
//!   cargo xtask hooks       install the git hooks, once per clone
//!   cargo xtask commit-msg  the shape of a commit subject (the hook calls this)
//! ```
//!
//! Everything this project uses is reached through here, including tools that
//! live outside the workspace. A second way to run a check is a second thing to
//! keep in step with CI, and it never is.
//!
//! # Why not a Makefile
//!
//! The invariant check reads a registry, walks the source tree and reports which
//! anchor in which file is wrong. That is a program. Written in make it would be
//! a program with no types, no tests and no compiler — and it is the thing that
//! guards the guarantees the product is sold on.

// This is a command line tool. Reporting to the terminal is its output, not a
// debugging leftover.
#![allow(clippy::print_stderr, reason = "the gate reports what it is doing")]

use std::process::ExitCode;

mod book;
mod budget;
mod commit;
mod docs;
mod invariants;
mod lint;
mod live;
mod shell;

const USAGE: &str = "\
usage: cargo xtask <task>

tasks:
  ci          fmt, lint, test, invariants — what CI runs
  fmt         check formatting
  lint        clippy with warnings denied, then repository rules
  test        the suites, with and without contracts
  invariants  doc/invariant/ against the code, in both directions
  docs        decision records, then rustdoc with warnings denied
  deny        dependency licences and advisories
  mutants     mutation testing (minutes; not part of ci)
  book        assemble the book's index from what is already written
  budget      what a plan costs, against docs/budgets.toml (not part of ci)
  live        build, install and exercise the system against a real fleet
  commit-msg  the shape of a commit subject; takes a file, or judges HEAD
  hooks       point git at .githooks, so commit-msg runs where it is written
";

fn main() -> ExitCode {
    let task = std::env::args().nth(1);
    let outcome = match task.as_deref() {
        Some("ci") => ci(),
        Some("fmt") => fmt(),
        Some("lint") => lint(),
        Some("live") => live::run(),
        Some("test") => test(),
        Some("invariants") => invariants::check(),
        Some("docs") => docs::check(),
        Some("deny") => deny(),
        Some("book") => book::check(),
        Some("budget") => budget::check(),
        Some("commit-msg") => commit::check(std::env::args().nth(2).as_deref()),
        Some("hooks") => hooks(),
        Some("mutants") => mutants(),
        Some(unknown) => Err(format!("unknown task {unknown:?}\n\n{USAGE}")),
        None => Err(USAGE.to_owned()),
    };

    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(report) => {
            eprintln!("{report}");
            ExitCode::FAILURE
        }
    }
}

fn ci() -> Result<(), String> {
    fmt()?;
    lint()?;
    test()?;
    invariants::check()?;
    book::check()?;
    docs::check()?;
    deny()
}

/// Points git at the hooks kept in this repository.
///
/// Hooks are not cloned, so every working copy starts without them. One command
/// rather than a line in a document somebody reads once: a convention that has
/// to be installed by hand is a convention half the clones do not have.
fn hooks() -> Result<(), String> {
    shell::git(&["config", "core.hooksPath", ".githooks"])?;
    eprintln!("  hooks: git will now run .githooks for this clone");
    Ok(())
}

fn deny() -> Result<(), String> {
    shell::cargo_tool("deny", "cargo install cargo-deny --locked", &["check"])
}

/// Deliberately not part of `ci`: it is minutes rather than seconds, and a gate
/// people learn to skip is worse than no gate. Nightly in CI, by hand when a
/// test suite has just been reorganised.
fn mutants() -> Result<(), String> {
    shell::cargo_tool("mutants", "cargo install cargo-mutants --locked", &[])
}

fn fmt() -> Result<(), String> {
    shell::cargo(&["fmt", "--all", "--", "--check"])
}

fn lint() -> Result<(), String> {
    shell::cargo_with_warnings_denied(&["clippy", "--all-targets", "--all-features"])?;
    lint::repository_rules()
}

fn test() -> Result<(), String> {
    shell::cargo(&["test", "--all-features"])?;
    // Contracts are compiled in by default. Building without them proves the
    // code does not quietly depend on their side effects.
    shell::cargo(&["test", "--no-default-features"])
}
