//! Running cargo, and reporting what happened when it fails.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The repository root, derived from this crate's location rather than from the
/// current directory, so a task behaves the same wherever it is invoked from.
pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask sits one level below the repository root")
        .to_path_buf()
}

pub fn cargo(args: &[&str]) -> Result<(), String> {
    run(args, false)
}

pub fn cargo_with_warnings_denied(args: &[&str]) -> Result<(), String> {
    run(args, true)
}

fn run(args: &[&str], deny_warnings: bool) -> Result<(), String> {
    eprintln!("  cargo {}", args.join(" "));

    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let mut command = Command::new(cargo);
    command.current_dir(root()).args(args);
    if deny_warnings {
        command.env("RUSTFLAGS", "-D warnings");
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
