//! Running cargo, and reporting what happened when it fails.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The repository root, derived from this crate's location rather than from the
/// current directory, so a task behaves the same wherever it is invoked from.
pub(crate) fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask sits one level below the repository root")
        .to_path_buf()
}

/// Runs git and hands back what it printed.
///
/// Separate from [`cargo`] because the output is the answer here rather than a
/// side effect: a hook needs the message, not an exit code.
pub(crate) fn git(args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .current_dir(root())
        .args(args)
        .output()
        .map_err(|error| format!("could not run git {}: {error}", args.join(" ")))?;

    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub(crate) fn cargo(args: &[&str]) -> Result<(), String> {
    run(args, Denied::None)
}

pub(crate) fn cargo_with_warnings_denied(args: &[&str]) -> Result<(), String> {
    run(args, Denied::Rust)
}

pub(crate) fn cargo_with_doc_warnings_denied(args: &[&str]) -> Result<(), String> {
    run(args, Denied::Rustdoc)
}

/// Which warnings the invocation must treat as failures.
#[derive(Debug, Clone, Copy)]
enum Denied {
    None,
    Rust,
    Rustdoc,
}

fn run(args: &[&str], denied: Denied) -> Result<(), String> {
    eprintln!("  cargo {}", args.join(" "));

    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let mut command = Command::new(cargo);
    command.current_dir(root()).args(args);
    match denied {
        Denied::None => {}
        Denied::Rust => {
            command.env("RUSTFLAGS", "-D warnings");
        }
        Denied::Rustdoc => {
            command.env("RUSTDOCFLAGS", "-D warnings");
        }
    }

    let status = command
        .status()
        .map_err(|error| format!("could not run cargo {}: {error}", args.join(" ")))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("cargo {} failed", args.join(" ")))
    }
}

/// Runs an external cargo subcommand, saying how to get it when it is absent.
///
/// Every tool this project uses is reached through `cargo xtask`, so a missing
/// one has to explain itself here rather than fail with "no such command".
pub(crate) fn cargo_tool(subcommand: &str, install: &str, args: &[&str]) -> Result<(), String> {
    let mut all = vec![subcommand];
    all.extend_from_slice(args);

    // Checked before running, not after. Deciding this from a failed run means a
    // tool that ran fine and found real problems gets reported as missing, which
    // is how the first mutation run described itself.
    if !installed(subcommand) {
        return Err(format!(
            "cargo-{subcommand} is not installed. Get it with:\n    {install}"
        ));
    }
    run(&all, Denied::None)
}

/// Asks cargo, not the binary.
///
/// A cargo subcommand is invoked as `cargo-thing thing ...` — cargo passes its
/// own name through as the first argument, and several of them reject being run
/// without it. Probing `cargo-thing --version` therefore reports a perfectly
/// well installed tool as missing, which is what this function did on its first
/// outing and why a mutation run silently never happened.
///
/// `--version` first, because a tool that has one answers it. cargo-dacc's
/// contract has no `--version` — it refuses with a usage code — so anything
/// that refuses the first question is asked its read-only answer, `help`, and
/// a tool that is genuinely missing refuses both.
fn installed(subcommand: &str) -> bool {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let asks = |args: &[&str]| {
        Command::new(&cargo)
            .args(args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    };
    asks(&[subcommand, "--version"]) || asks(&[subcommand, "help"])
}
