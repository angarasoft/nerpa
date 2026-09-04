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
//!   cargo xtask invariants  docs/invariants.toml against the code, both ways
//!   cargo xtask docs        decision records, then rustdoc with warnings denied
//! ```
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

mod docs;
mod invariants;
mod lint;
mod shell;

const USAGE: &str = "\
usage: cargo xtask <task>

tasks:
  ci          fmt, lint, test, invariants — what CI runs
  fmt         check formatting
  lint        clippy with warnings denied, then repository rules
  test        the suites, with and without contracts
  invariants  docs/invariants.toml against the code, in both directions
  docs        decision records, then rustdoc with warnings denied
";

fn main() -> ExitCode {
    let task = std::env::args().nth(1);
    let outcome = match task.as_deref() {
        Some("ci") => ci(),
        Some("fmt") => fmt(),
        Some("lint") => lint(),
        Some("test") => test(),
        Some("invariants") => invariants::check(),
        Some("docs") => docs::check(),
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
    docs::check()
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
