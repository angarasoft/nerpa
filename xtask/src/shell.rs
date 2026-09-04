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
