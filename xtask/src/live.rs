//! Running the built system against real machines, not just in unit tests.
//!
//! The gate proves only the unit tests: nothing there runs the engine, the wire
//! and the os provider against a real machine. This command builds the binaries,
//! puts the static executor on each machine named by a fleet, and runs a
//! configuration against them — `plan`, `apply`, then `plan` again, which must
//! report nothing to do.
//!
//! ```text
//! cargo xtask live --config <config.star> --fleet <fleet.toml>
//! ```
//!
//! The fleet is the document `nerpa` reads. Each `[nodes.<name>]` reached over
//! `ssh` says `ssh`, optionally `ssh_config`, and — for the privileged case —
//! `executor` and `become`. The executor is installed where `executor` names: a
//! path root owns, so `become` is root wearing a narrow rule rather than full
//! root (decisions 0039 and 0040). A node without an `executor` is delivered to
//! by `nerpa` itself, into the connecting user's directory, and is left alone
//! here.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use toml::Value;

use crate::shell;

/// One machine to put the executor on and run against, read from the fleet.
#[derive(Debug)]
struct Machine {
    node: String,
    ssh: String,
    ssh_config: Option<String>,
    executor: String,
    escalate: Vec<String>,
}

/// Where the three binaries sit beside each other, the way `nerpa` expects.
fn staging() -> PathBuf {
    shell::root().join("target").join("live")
}

/// Builds the command, the local executor and the static one, and stages them.
fn build() -> Result<PathBuf, String> {
    shell::cargo(&[
        "build",
        "--release",
        "--bin",
        "nerpa",
        "--bin",
        "nerpa-executor",
    ])?;
    shell::cargo(&[
        "build",
        "--release",
        "--target",
        "x86_64-unknown-linux-musl",
        "--bin",
        "nerpa-executor",
    ])?;

    let stage = staging();
    fs::create_dir_all(&stage).map_err(|error| format!("{}: {error}", stage.display()))?;

    let root = shell::root();
    let placed = [
        ("nerpa", root.join("target/release/nerpa")),
        ("nerpa-executor", root.join("target/release/nerpa-executor")),
        (
            "nerpa-executor.static",
            root.join("target/x86_64-unknown-linux-musl/release/nerpa-executor"),
        ),
    ];
    for (name, from) in placed {
        fs::copy(&from, stage.join(name))
            .map_err(|error| format!("{}: {error}", from.display()))?;
    }
    Ok(stage)
}

/// Reads the machines reached over `ssh`, each with an `executor` to install to.
fn read_fleet(path: &Path) -> Result<Vec<Machine>, String> {
    let text = fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let parsed: toml::Table =
        toml::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))?;
    let nodes = parsed
        .get("nodes")
        .and_then(Value::as_table)
        .ok_or_else(|| format!("{}: no [nodes] table", path.display()))?;

    let mut machines = Vec::new();
    for (node, value) in nodes {
        let table = value
            .as_table()
            .ok_or_else(|| format!("{}: [nodes.{node}] is not a table", path.display()))?;
        // A node reached over `ssh` says so. One that does not is the local
        // machine, and needs no executor put there.
        let Some(ssh) = table.get("ssh").and_then(Value::as_str) else {
            continue;
        };
        // A node without an `executor` is delivered to by `nerpa` itself; there
        // is nothing to install for it here.
        let Some(executor) = table.get("executor").and_then(Value::as_str) else {
            continue;
        };
        let ssh_config = table
            .get("ssh_config")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let escalate = table
            .get("become")
            .and_then(Value::as_str)
            .map_or_else(Vec::new, |words| {
                words.split_whitespace().map(str::to_owned).collect()
            });
        machines.push(Machine {
            node: node.clone(),
            ssh: ssh.to_owned(),
            ssh_config,
            executor: executor.to_owned(),
            escalate,
        });
    }
    Ok(machines)
}

/// Puts the static executor on each machine, whole or not at all.
fn install(stage: &Path, machines: &[Machine]) -> Result<(), String> {
    let binary = fs::read(stage.join("nerpa-executor.static"))
        .map_err(|error| format!("nerpa-executor.static: {error}"))?;

    for machine in machines {
        // Written beside, then moved into place, so a copy cut short is never
        // under the executor's name.
        let remote = format!(
            "cat > /tmp/nerpa-executor.install && {} install -m 0755 /tmp/nerpa-executor.install {} && rm -f /tmp/nerpa-executor.install",
            machine.escalate.join(" "),
            machine.executor,
        );

        let mut ssh = Command::new("ssh");
        ssh.arg("-o").arg("BatchMode=yes");
        if let Some(config) = &machine.ssh_config {
            ssh.arg("-F").arg(config);
        }
        ssh.current_dir(shell::root())
            .arg(&machine.ssh)
            .arg(&remote)
            .stdin(Stdio::piped());

        let mut child = ssh
            .spawn()
            .map_err(|error| format!("{}: could not run ssh: {error}", machine.node))?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| format!("{}: ssh stdin is not piped", machine.node))?;
        stdin
            .write_all(&binary)
            .map_err(|error| format!("{}: could not send the executor: {error}", machine.node))?;
        // Close stdin so the far side's `cat` sees the end of the file and
        // finishes, rather than waiting for more that is never coming.
        drop(stdin);
        let status = child
            .wait()
            .map_err(|error| format!("{}: {error}", machine.node))?;
        if !status.success() {
            return Err(format!("{}: the executor was not installed", machine.node));
        }
        eprintln!("  live: executor installed on {}", machine.node);
    }
    Ok(())
}

/// Runs `plan`, `apply`, then `plan` again; the last must report nothing to do.
fn exercise(stage: &Path, config: &Path, fleet: &Path) -> Result<(), String> {
    let nerpa = stage.join("nerpa");
    let state_file = stage.join("live.state.json");

    for verb in ["plan", "apply"] {
        let status = Command::new(&nerpa)
            .current_dir(shell::root())
            .arg("-f")
            .arg(config)
            .arg("--fleet")
            .arg(fleet)
            .arg("--state")
            .arg(&state_file)
            .arg(verb)
            .status()
            .map_err(|error| format!("could not run {}: {error}", nerpa.display()))?;
        if !status.success() {
            return Err(format!("nerpa {verb} failed"));
        }
    }

    let output = Command::new(&nerpa)
        .current_dir(shell::root())
        .arg("-f")
        .arg(config)
        .arg("--fleet")
        .arg(fleet)
        .arg("--state")
        .arg(&state_file)
        .arg("plan")
        .output()
        .map_err(|error| format!("could not run {}: {error}", nerpa.display()))?;
    if !output.status.success() {
        return Err("nerpa plan failed after applying".to_owned());
    }
    let said = String::from_utf8_lossy(&output.stdout);
    if !said.contains("nothing to do") {
        return Err("the second plan is not empty: the world did not converge".to_owned());
    }
    eprintln!("  live: converged — the second plan reports nothing to do");
    Ok(())
}

/// `cargo xtask live --config <config.star> --fleet <fleet.toml>`.
pub(crate) fn run() -> Result<(), String> {
    let (config, fleet) = arguments()?;
    let stage = build()?;
    let machines = read_fleet(&fleet)?;
    install(&stage, &machines)?;
    exercise(&stage, &config, &fleet)
}

/// Reads `--config` and `--fleet` from the command line after the task name.
fn arguments() -> Result<(PathBuf, PathBuf), String> {
    let mut config = None;
    let mut fleet = None;
    let mut args = std::env::args().skip(2);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--config" => {
                config = Some(PathBuf::from(args.next().ok_or("--config needs a file")?));
            }
            "--fleet" => {
                fleet = Some(PathBuf::from(args.next().ok_or("--fleet needs a file")?));
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    let config = config.ok_or("--config <config.star> is required")?;
    let fleet = fleet.ok_or("--fleet <fleet.toml> is required")?;
    Ok((config, fleet))
}

// Both directions of the fleet read are shown a fixture, so the one place the
// harness trusts the document's shape is watched.
#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "tests may be blunt: see docs/CODING_STANDARDS.md"
)]
mod tests {
    use super::read_fleet;
    use std::path::Path;

    #[test]
    fn a_fleet_reads_the_machines_reached_over_ssh_with_an_executor() {
        let dir = std::env::temp_dir().join("nerpa-live-fleet.toml");
        std::fs::write(
            &dir,
            r#"
[nodes.alpine]
ssh = "alpine@10.0.61.41"
executor = "/usr/local/sbin/nerpa-executor"
become = "sudo -n"

[nodes.delivered]
ssh = "web-01"

[nodes.local]
"#,
        )
        .unwrap();

        let machines = read_fleet(Path::new(&dir)).unwrap();
        assert_eq!(machines.len(), 1, "{machines:?}");
        assert_eq!(machines[0].node, "alpine");
        assert_eq!(
            machines[0].escalate,
            vec!["sudo".to_owned(), "-n".to_owned()]
        );
        assert_eq!(machines[0].executor, "/usr/local/sbin/nerpa-executor");
    }
}
