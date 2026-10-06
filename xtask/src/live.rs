//! Running the built system against real machines, not just in unit tests.
//!
//! The gate proves only the unit tests: nothing there runs the engine, the wire
//! and the os provider against a real machine. This command builds the binaries,
//! puts the static executor on each machine named by a fleet, and runs against
//! them. Two modes:
//!
//! ```text
//! cargo xtask live --config <config.star> --fleet <fleet.toml>
//! cargo xtask live --stage4 --fleet <fleet.toml>
//! ```
//!
//! The first runs a configuration as `plan`, `apply`, then `plan` again, and
//! the last must print the sentence the renderer gives a converged world. The
//! second is the stage-4 scenario: the five verbs the thrust's outcome is made
//! of — `plan --json`, `sign` as a second party with a key of its own, `apply`
//! over the approval, the world moving under the signature, `apply` again
//! refused naming what moved, and `journal verify`. It is marked as needing the
//! live park, because ssh reach to real machines is what makes it a stage exit
//! rather than a test.
//!
//! The fleet is the document `nerpa` reads. Each `[nodes.<name>]` reached over
//! `ssh` says `ssh`, optionally `ssh_config`, and — for the privileged case —
//! `executor` and `become`. The executor is installed where `executor` names: a
//! path root owns, so `become` is root wearing a narrow rule rather than full
//! root (decisions 0039 and 0040). A node without an `executor` is delivered to
//! by `nerpa` itself, into the connecting user's directory, and is left alone
//! here.

use std::fmt::Write as _;
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

/// What the harness runs: the convergence round, or the stage-4 scenario.
struct Invocation {
    /// The configuration to run, for the convergence round.
    config: Option<PathBuf>,
    /// The fleet: the machines there are, and how to reach them.
    fleet: PathBuf,
    /// Whether to drive the stage-4 scenario instead.
    stage4: bool,
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

    let target = shell::target_dir();
    let placed = [
        ("nerpa", target.join("release/nerpa")),
        ("nerpa-executor", target.join("release/nerpa-executor")),
        (
            "nerpa-executor.static",
            target.join("x86_64-unknown-linux-musl/release/nerpa-executor"),
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

/// A name of this run's own: the process and the clock, which is all one run
/// has that no other run has.
fn nonce() -> String {
    let since = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.subsec_nanos());
    format!("{}.{}", std::process::id(), since)
}

/// The far side of an install: the whole shell line that receives the executor,
/// stages it beside its destination, and moves it into place.
///
/// Written beside, then moved into place, so a copy cut short is never under
/// the executor's name. The staging file is `remote` — a name of the run's own,
/// not the fixed `/tmp/nerpa-executor.install` a predictable name in a shared
/// /tmp deserves: one identity writes it and root consumes it, so the name is
/// the only thing standing between the two.
fn install_command(escalate: &[String], executor: &str, remote: &str) -> String {
    format!(
        "cat > {remote} && {} install -m 0755 {remote} {executor} && rm -f {remote}",
        escalate.join(" "),
    )
}

/// Puts the static executor on each machine, whole or not at all.
fn install(stage: &Path, machines: &[Machine]) -> Result<(), String> {
    let binary = fs::read(stage.join("nerpa-executor.static"))
        .map_err(|error| format!("nerpa-executor.static: {error}"))?;
    let nonce = nonce();

    for machine in machines {
        let remote = format!("/tmp/nerpa-executor.install.{nonce}");
        let command = install_command(&machine.escalate, &machine.executor, &remote);

        let mut ssh = Command::new("ssh");
        ssh.arg("-o").arg("BatchMode=yes");
        if let Some(config) = &machine.ssh_config {
            ssh.arg("-F").arg(config);
        }
        ssh.current_dir(shell::root())
            .arg(&machine.ssh)
            .arg(&command)
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

/// Whether a plan says the world already matches the configuration.
///
/// The whole line, and the one line: the renderer gives the converged world a
/// sentence of its own. Matching a fragment let a plan that reports work count
/// as converged because it merely mentioned the words, and let a wording change
/// pass silently — both now stop the run, which is the safer mistake for each.
fn converged(said: &str) -> bool {
    said.lines()
        .any(|line| line.trim_end() == "nothing to do: the world already matches the configuration")
}

/// Runs `plan`, `apply`, then `plan` again; the last must say the world already
/// matches the configuration.
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
    if !converged(&said) {
        return Err("the second plan is not empty: the world did not converge".to_owned());
    }
    eprintln!("  live: converged — the second plan reports nothing to do");
    Ok(())
}

/// The five verbs of the stage-4 outcome, in the order the scenario drives
/// them, and what each one is for.
fn stage4_verbs() -> [&'static str; 5] {
    [
        "plan --json, as the proposer",
        "sign --as the third party, with a key of its own",
        "apply --approval: the signed plan goes through",
        "apply --approval after an out-of-band change: refused, naming what moved",
        "journal verify: the log verifies from the first entry",
    ]
}

/// The scenario's own configuration: one file per machine, at a name of this
/// run's own.
///
/// The scenario has to know what it manages in order to change it out of band,
/// and an operator's configuration is whatever the operator needed that week.
/// So the stage-4 run writes its own from the fleet it is given.
fn scenario_config(machines: &[Machine], name: &str) -> Result<String, String> {
    if machines.is_empty() {
        return Err(
            "the fleet names no machine reached over ssh with an executor; the \
             stage-4 scenario runs against the live park"
                .to_owned(),
        );
    }
    let mut text = String::new();
    for machine in machines {
        let _ = writeln!(text, "node({:?})", machine.node);
        let _ = write!(
            text,
            "resource(\"os:file.stage4@{}\", {{\n    \"path\": {:?},\n    \"content\": \"one\\n\",\n}})\n",
            machine.node, name
        );
    }
    Ok(text)
}

/// The operator's fleet document with the one section the scenario adds: who
/// may sign. Everything else — how a host is reached, where its executor is —
/// stays exactly as the operator wrote it.
fn with_approvers(fleet: &str, approvers: &Path) -> Result<String, String> {
    if fleet.contains("[approval]") {
        return Err(
            "the stage-4 scenario writes its own [approval] naming its own signer, \
             and this fleet already names one; run it with a fleet that leaves \
             signing to the scenario"
                .to_owned(),
        );
    }
    Ok(format!(
        "{fleet}\n[approval]\napprovers = {:?}\n",
        approvers.display().to_string()
    ))
}

/// An ed25519 key in the harness's own directory: made here, used here, and
/// removed here (`0067`). `ssh-keygen` reads the key; nothing in nerpa does.
fn keygen(path: &Path) -> Result<String, String> {
    let made = Command::new("ssh-keygen")
        .args(["-q", "-t", "ed25519", "-N", ""])
        .arg("-f")
        .arg(path)
        .status()
        .map_err(|error| format!("ssh-keygen could not be run: {error}"))?;
    if !made.success() {
        return Err("ssh-keygen refused to make the scenario's key".to_owned());
    }
    public_line(&path.with_extension("pub"))
}

/// The public half of a key, as one line of an `allowed_signers` file holds it.
fn public_line(path: &Path) -> Result<String, String> {
    let said = fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let parts: Vec<&str> = said.split_whitespace().take(2).collect();
    match parts.as_slice() {
        [kind, body] => Ok(format!("{kind} {body}")),
        _ => Err(format!(
            "{}: this is not a public key this build can read",
            path.display()
        )),
    }
}

/// The out-of-band change: the managed file written by hand, through the same
/// door a person with a shell on the machine would use.
fn moved_by_hand(machine: &Machine, name: &str) -> Result<(), String> {
    let edit = if machine.escalate.is_empty() {
        format!("echo moved-by-hand > {name}")
    } else {
        format!(
            "{} sh -c \"echo moved-by-hand > {name}\"",
            machine.escalate.join(" ")
        )
    };
    let mut ssh = Command::new("ssh");
    ssh.arg("-o").arg("BatchMode=yes");
    if let Some(config) = &machine.ssh_config {
        ssh.arg("-F").arg(config);
    }
    let status = ssh
        .current_dir(shell::root())
        .arg(&machine.ssh)
        .arg(&edit)
        .status()
        .map_err(|error| format!("{}: could not run ssh: {error}", machine.node))?;
    if !status.success() {
        return Err(format!(
            "{}: the out-of-band change did not go through",
            machine.node
        ));
    }
    Ok(())
}

/// The stage-4 exit evidence, driven as a person drives it.
///
/// The run the thrust's outcome is made of, in one sequence: the plan to sign,
/// the signature of a second party over it, the apply that goes through, the
/// world moving under the signature, the apply that is refused naming what
/// moved, and the log that verifies from the first entry. It needs ssh reach to
/// the live park and says so before it starts.
#[allow(
    clippy::too_many_lines,
    reason = "the scenario's steps stay in one place and in order, the way a \
              person runs them"
)]
fn exercise_stage4(stage: &Path, machines: &[Machine], fleet: &Path) -> Result<(), String> {
    eprintln!(
        "  live: stage 4 needs the live park — ssh reach to every machine of \
         the fleet, and an executor path root owns on each"
    );
    for (at, verb) in stage4_verbs().iter().enumerate() {
        eprintln!(
            "  live: stage 4, step {} of {}: {verb}",
            at + 1,
            stage4_verbs().len()
        );
    }

    let nonce = nonce();
    let keys = stage.join(format!("stage4-keys-{nonce}"));
    fs::create_dir_all(&keys).map_err(|error| format!("{}: {error}", keys.display()))?;

    // The two duties, and one key between them: whoever proposes the plan holds
    // no key at all, and the signer's key lives only as long as the scenario.
    let proposer = "alice@example.com";
    let signer = "carol@example.com";
    let key = keys.join("signer");
    let public = keygen(&key)?;
    let approvers = keys.join("approvers");
    fs::write(&approvers, format!("{signer} {public}\n"))
        .map_err(|error| format!("{}: {error}", approvers.display()))?;

    let fleet_text =
        fs::read_to_string(fleet).map_err(|error| format!("{}: {error}", fleet.display()))?;
    let derived = stage.join(format!("stage4-fleet-{nonce}.toml"));
    fs::write(&derived, with_approvers(&fleet_text, &approvers)?)
        .map_err(|error| format!("{}: {error}", derived.display()))?;

    let name = format!("/tmp/nerpa-stage4.{nonce}.note");
    let config = stage.join(format!("stage4-config-{nonce}.star"));
    fs::write(&config, scenario_config(machines, &name)?)
        .map_err(|error| format!("{}: {error}", config.display()))?;

    let nerpa = stage.join("nerpa");
    let state_file = stage.join(format!("stage4-state-{nonce}.json"));
    let plan = stage.join(format!("stage4-plan-{nonce}.json"));
    let approval = stage.join(format!("stage4-plan-{nonce}.json.approval"));
    let scenario = |verb: &str| {
        let mut command = Command::new(&nerpa);
        command
            .current_dir(shell::root())
            .arg("-f")
            .arg(&config)
            .arg("--fleet")
            .arg(&derived)
            .arg("--state")
            .arg(&state_file)
            .arg(verb);
        command
    };

    // 1. plan --json, as the proposer: the plan that will be signed.
    let planned = scenario("plan")
        .arg("--json")
        .arg("--as")
        .arg(proposer)
        .output()
        .map_err(|error| format!("could not run {}: {error}", nerpa.display()))?;
    if !planned.status.success() {
        return Err(format!(
            "nerpa plan --json failed: {}",
            String::from_utf8_lossy(&planned.stderr)
        ));
    }
    fs::write(&plan, &planned.stdout).map_err(|error| format!("{}: {error}", plan.display()))?;

    // 2. sign, as the third party, with a key of its own.
    let wrote = scenario("sign")
        .arg(&plan)
        .arg("--as")
        .arg(signer)
        .arg("--with-key")
        .arg(&key)
        .arg("--out")
        .arg(&approval)
        .status()
        .map_err(|error| format!("could not run {}: {error}", nerpa.display()))?;
    if !wrote.success() {
        return Err("nerpa sign failed: the third party would not sign".to_owned());
    }

    // 3. apply over the approval: the signed plan goes through.
    let applied = scenario("apply")
        .arg("--as")
        .arg(proposer)
        .arg("--approval")
        .arg(&approval)
        .status()
        .map_err(|error| format!("could not run {}: {error}", nerpa.display()))?;
    if !applied.success() {
        return Err("the apply over the signed plan failed".to_owned());
    }
    eprintln!("  live: stage 4, the signed plan went through");

    // 4. the world moves under the signature: the managed file written by hand.
    let machine = machines
        .first()
        .ok_or_else(|| "the scenario managed no machine; the live park is missing".to_owned())?;
    moved_by_hand(machine, &name)?;

    // 5. the same approval again, refused, naming what moved.
    let refused = scenario("apply")
        .arg("--as")
        .arg(proposer)
        .arg("--approval")
        .arg(&approval)
        .output()
        .map_err(|error| format!("could not run {}: {error}", nerpa.display()))?;
    if refused.status.success() {
        return Err(
            "the apply after the out-of-band change went through: the signature \
             was taken before it, and the plan it covers is not this one"
                .to_owned(),
        );
    }
    let problem = String::from_utf8_lossy(&refused.stderr);
    if !(problem.contains("moved") && problem.contains("os:file.stage4@")) {
        return Err(format!("the refusal does not name what moved: {problem}"));
    }
    eprintln!("  live: stage 4, the refusal names what moved");

    // 6. journal verify: the log verifies from the first entry.
    let verified = scenario("journal")
        .arg("verify")
        .arg("--as")
        .arg(proposer)
        .status()
        .map_err(|error| format!("could not run {}: {error}", nerpa.display()))?;
    if !verified.success() {
        return Err("the run journal does not verify".to_owned());
    }

    // The keys were the scenario's, and the scenario is over. What it leaves
    // behind — the plan, the approval, state and the journal — is the evidence.
    let _ = fs::remove_dir_all(&keys);
    eprintln!(
        "  live: stage 4 evidence kept in {} — the plan, the approval, state and the journal",
        stage.display()
    );
    Ok(())
}

/// `cargo xtask live --config <config.star> --fleet <fleet.toml>` — or
/// `--stage4 --fleet <fleet.toml>` for the stage-4 scenario.
pub(crate) fn run() -> Result<(), String> {
    let arguments = arguments()?;
    let stage = build()?;
    let machines = read_fleet(&arguments.fleet)?;
    install(&stage, &machines)?;
    if arguments.stage4 {
        return exercise_stage4(&stage, &machines, &arguments.fleet);
    }
    let config = arguments
        .config
        .ok_or("--config <config.star> is required")?;
    exercise(&stage, &config, &arguments.fleet)
}

/// Reads `--config`, `--fleet` and `--stage4` from the command line after the
/// task name.
fn arguments() -> Result<Invocation, String> {
    let mut config = None;
    let mut fleet = None;
    let mut stage4 = false;
    let mut args = std::env::args().skip(2);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--config" => {
                config = Some(PathBuf::from(args.next().ok_or("--config needs a file")?));
            }
            "--fleet" => {
                fleet = Some(PathBuf::from(args.next().ok_or("--fleet needs a file")?));
            }
            "--stage4" => stage4 = true,
            other => return Err(format!("unknown argument {other}")),
        }
    }
    let fleet = fleet.ok_or("--fleet <fleet.toml> is required")?;
    if !stage4 && config.is_none() {
        return Err("--config <config.star> is required".to_owned());
    }
    Ok(Invocation {
        config,
        fleet,
        stage4,
    })
}

// Each piece the scenario is composed from is shown a fixture, so the run
// against the live park is not the first place their shapes are judged.
#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "tests may be blunt: see docs/CODING_STANDARDS.md"
)]
mod tests {
    use super::{
        Machine, converged, install_command, read_fleet, scenario_config, stage4_verbs,
        with_approvers,
    };
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

    /// The substring this used to match let a plan that reports work count as
    /// converged. It counts as nothing now.
    #[test]
    fn a_plan_that_merely_mentions_the_words_is_not_converged() {
        assert!(
            !converged("there is nothing to do about this\n"),
            "a fragment is not the sentence"
        );
        assert!(
            !converged("nothing to do about what could be read\n"),
            "the empty plan with reports left is not the converged world"
        );
        assert!(converged(
            "os:file.a@localhost: created\nnothing to do: the world already matches the configuration\n"
        ));
    }

    #[test]
    fn the_install_goes_through_a_name_of_this_runs_own() {
        let one = install_command(
            &["sudo".to_owned(), "-n".to_owned()],
            "/usr/local/sbin/nerpa-executor",
            "/tmp/nerpa-executor.install.11.22",
        );
        assert!(
            one.contains(
                "cat > /tmp/nerpa-executor.install.11.22 && sudo -n install -m 0755 \
                 /tmp/nerpa-executor.install.11.22 /usr/local/sbin/nerpa-executor && \
                 rm -f /tmp/nerpa-executor.install.11.22"
            ),
            "{one}"
        );
        assert!(
            !one.contains("/tmp/nerpa-executor.install &&"),
            "the fixed name is gone: {one}"
        );
    }

    fn machine(node: &str) -> Machine {
        Machine {
            node: node.to_owned(),
            ssh: format!("{node}@10.0.61.41"),
            ssh_config: None,
            executor: "/usr/local/sbin/nerpa-executor".to_owned(),
            escalate: vec!["sudo".to_owned(), "-n".to_owned()],
        }
    }

    #[test]
    fn the_scenario_manages_one_file_per_machine_at_a_name_of_its_own() {
        let text =
            scenario_config(&[machine("alpine"), machine("rocky")], "/tmp/x.1.2.note").unwrap();
        assert!(
            text.contains("node(\"alpine\")") && text.contains("node(\"rocky\")"),
            "{text}"
        );
        assert!(
            text.contains("os:file.stage4@alpine") && text.contains("os:file.stage4@rocky"),
            "{text}"
        );
        assert!(text.contains("\"/tmp/x.1.2.note\""), "{text}");
        assert!(
            scenario_config(&[], "/tmp/x.1.2.note").is_err(),
            "the scenario runs against machines, and says so when there are none"
        );
    }

    #[test]
    fn the_scenario_fleet_names_its_approvers_file() {
        let text = with_approvers(
            "[nodes.alpine]\nssh = \"alpine@10.0.61.41\"\n",
            Path::new("/tmp/approvers"),
        )
        .unwrap();
        assert!(
            text.contains("[approval]") && text.contains("approvers"),
            "{text}"
        );
        assert!(
            text.contains("ssh = \"alpine@10.0.61.41\""),
            "the operator's document is kept whole: {text}"
        );
        assert!(
            with_approvers(
                "[approval]\napprovers = \"x\"\n",
                Path::new("/tmp/approvers")
            )
            .is_err(),
            "a fleet that already names its signers is not quietly rewritten"
        );
    }

    /// The five verbs the thrust's outcome is made of, in order — the shape the
    /// scenario exists to produce.
    #[test]
    fn the_stage4_run_drives_the_five_verbs_of_the_outcome() {
        assert_eq!(
            stage4_verbs().to_vec(),
            vec![
                "plan --json, as the proposer",
                "sign --as the third party, with a key of its own",
                "apply --approval: the signed plan goes through",
                "apply --approval after an out-of-band change: refused, naming what moved",
                "journal verify: the log verifies from the first entry",
            ]
        );
    }
}
