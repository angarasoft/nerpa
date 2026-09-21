//! The `nerpa` command.
//!
//! Two verbs, and no third one that quietly does both. `plan` says what would
//! happen; `apply` makes it happen and writes down what it did.
//!
//! # How the pieces fit
//!
//! ```text
//!   main.star ── evaluate ── Graph
//!                              │
//!   state.json ── load ────────┤
//!                              ├── observe ── Observed
//!                              │                 │
//!                              └──────── plan ───┴── Plan ── print
//!                                                        └── apply ── state.json
//! ```

// A command line tool. Writing to the terminal is its output.
#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "this is the terminal interface"
)]

mod render;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use nerpa_config::fleet::{Fleet, Route};
use nerpa_core::address::Target;
use nerpa_core::planner;
use nerpa_core::provider::{Machines, Providers, Reach};
use nerpa_core::run;
use nerpa_core::secret::{Remembered, Secrets};
use nerpa_core::shards;
use nerpa_core::state::State;
use nerpa_provider_os::Os;

#[derive(Parser)]
#[command(
    name = "nerpa",
    version,
    about = "Infrastructure and configuration in one graph"
)]
struct Arguments {
    #[command(subcommand)]
    task: Task,

    /// The configuration to read.
    #[arg(short = 'f', long, default_value = "main.star", global = true)]
    file: PathBuf,

    /// Where the record of what was made is kept.
    #[arg(long, default_value = "nerpa.state.json", global = true)]
    state: PathBuf,

    /// Which machines there are, and what is written down about them.
    #[arg(long, global = true)]
    fleet: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Task {
    /// Say what would happen, and change nothing.
    Plan {
        /// Print the plan as JSON, for something that is not a person.
        #[arg(long)]
        json: bool,
    },
    /// Make it happen.
    Apply,
}

fn main() -> ExitCode {
    let arguments = Arguments::parse();
    match run_task(&arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(report) => {
            eprintln!("nerpa: {report}");
            ExitCode::FAILURE
        }
    }
}

/// What this build carries an executor for.
///
/// One target. A machine that is not this one is told so by name rather than
/// handed something it cannot run — building for several and choosing between
/// them is `W-008`.
const PORTABLE_ARCHITECTURE: &str = "x86_64";

/// The executor built to run somewhere that is not here, and the build it goes by.
///
/// A separate file from the one used locally, because a machine reached over
/// `ssh` is not this machine: the local build links against what is here, and a
/// static one is what travels. Needed for every machine reached over `ssh`, not
/// only the ones it is delivered to, because it is also what an installed
/// executor is compared with — decision 0040.
#[derive(Debug)]
struct Carried {
    /// Where it is, here.
    path: PathBuf,
    /// Its name, by [`nerpa_transport::build_of`].
    build: String,
}

/// Finds the executor this nerpa carries, and names it.
fn portable(beside: &Path) -> Result<Carried, String> {
    let path = beside.with_file_name("nerpa-executor.static");
    if !path.exists() {
        return Err(format!(
            "{} is not there. A machine reached over ssh is checked against the \
             executor this nerpa carries, and is given it when nothing names one",
            path.display()
        ));
    }
    let binary = std::fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(Carried {
        build: nerpa_transport::build_of(&binary),
        path,
    })
}

/// How each machine in this configuration is reached.
///
/// Every node gets a pipe to an executor. Today that executor runs here, which
/// is what makes the whole path — provider, wire, executor — real on one machine
/// before there is a second one. `ssh` is the same call with a different command
/// in front of it, which is the point of decision 0032.
fn reachable(graph: &nerpa_core::Graph, fleet: &Fleet) -> Result<Machines, String> {
    let beside = std::env::current_exe()
        .map_err(|error| format!("this program cannot tell where it is: {error}"))?
        .parent()
        .map(|directory| directory.join("nerpa-executor"))
        .ok_or_else(|| "this program appears to have no directory".to_owned())?;

    if !beside.exists() {
        return Err(format!(
            "{} is not there; nerpa needs its executor beside it",
            beside.display()
        ));
    }

    let nodes: Vec<Target> = graph.inventory().nodes().cloned().collect();
    // Found once for the whole fleet. Not fatal here: a fleet reaching nothing
    // over `ssh` has no use for it, and one that does is refused machine by
    // machine, with the reason.
    let carried = portable(&beside);
    let (machines, refused) = registry(&nodes, &|node| speaking(node, fleet, &beside, &carried));
    for problem in refused {
        eprintln!("nerpa: {problem}");
    }
    Ok(machines)
}

/// How one machine is opened.
///
/// A parameter rather than a call, so the step can be failed on purpose. Named
/// because the shape of it is the point: everything that can go wrong because a
/// machine is not there goes wrong in here, and everything outside has to carry
/// on regardless.
type Opening<'a> = dyn Fn(&Target) -> Result<Box<dyn Reach>, String> + Sync + 'a;

// ENFORCES: INV-REACH-001
/// Opens a connection to each machine, and says which ones would not open.
///
/// The opening is a parameter rather than a call, because this is the step that
/// decides whether one machine being down ends a run — and until it took one, it
/// was the one step no test could reach. It was also wrong: a fleet with a
/// machine switched off hung, and every guarantee about unreachability lives
/// below here and never got asked.
///
/// A machine that will not open is left out. The planner then reports it as out
/// of reach, which is what it already knew how to do — see decision 0020.
fn registry(nodes: &[Target], open: &Opening<'_>) -> (Machines, Vec<String>) {
    // Side by side, because opening a machine waits for its handshake — it is
    // where the executor there is asked which build it is — and one at a time a
    // fleet would wait for every handshake in turn before reading anything.
    let opened = nerpa_core::fanout::every(nodes, open);
    let mut machines = Machines::new();
    let mut refused = Vec::new();
    for (node, outcome) in nodes.iter().zip(opened) {
        match outcome {
            Ok(reach) => machines = machines.with(node.clone(), reach),
            Err(problem) => refused.push(problem),
        }
    }
    (machines, refused)
}

/// Opens the connection to one machine.
///
/// Everything that can fail because that machine is not there fails here, so
/// the caller has one place to decide what an absent machine means.
fn speaking(
    node: &Target,
    fleet: &Fleet,
    beside: &Path,
    carried: &Result<Carried, String>,
) -> Result<Box<dyn Reach>, String> {
    let (program, arguments, expected) = match fleet.route(node) {
        // Not asked which build it is: the executor here is the file beside this
        // program, from the same build by construction.
        Route::Here => (beside.display().to_string(), Vec::new(), None),
        Route::Ssh {
            host,
            config,
            executor,
            escalate,
        } => {
            let carried = carried
                .as_ref()
                .map_err(|problem| format!("{node}: {problem}"))?;
            // Named by the fleet, or put there. A path somebody wrote down is
            // used as written: they know something about that machine that this
            // does not.
            let executor = if let Some(named) = executor {
                named
            } else {
                nerpa_transport::delivery::deliver(
                    node,
                    &host,
                    config.as_deref(),
                    &carried.path,
                    PORTABLE_ARCHITECTURE,
                )
                .map_err(|error| error.to_string())?
            };

            // `BatchMode` because a run that hangs waiting for a prompt nobody
            // will see is worse than one that fails: a fleet stops half way and
            // nothing says why. `ConnectTimeout` for the same reason, against a
            // machine that is switched off rather than one that asks a question.
            // `ServerAlive*` for the third shape of the same failure: a machine
            // that answered and then went away mid-run, which neither of the
            // other two sees.
            // Everything else about how this host is reached — the port, the
            // jump host, the key — belongs to ssh's own configuration and is
            // deliberately not repeated here.
            let mut arguments = vec![
                "-o".to_owned(),
                "BatchMode=yes".to_owned(),
                "-o".to_owned(),
                format!("ConnectTimeout={}", nerpa_transport::ANSWERS_WITHIN),
                "-o".to_owned(),
                format!("ServerAliveInterval={}", nerpa_transport::ALIVE_EVERY),
                "-o".to_owned(),
                format!("ServerAliveCountMax={}", nerpa_transport::ALIVE_MISSES),
            ];
            if let Some(named) = config {
                arguments.push("-F".to_owned());
                arguments.push(named);
            }
            arguments.push(host);
            arguments.extend(escalate);
            arguments.push(executor);
            ("ssh".to_owned(), arguments, Some(carried.build.as_str()))
        }
    };
    // ENFORCES: INV-WIRE-003
    // Which build answered is asked before anything is read, because what is
    // read is the executor's account of the machine, and an account from a build
    // this nerpa does not ship is not one it has a reason to believe. Opening
    // and that question are one step, because they are the part that can be
    // started over.
    let reach = nerpa_transport::opened(node, &program, &arguments, expected)
        .map_err(|error| error.to_string())?;
    Ok(Box::new(reach))
}

fn run_task(arguments: &Arguments) -> Result<(), String> {
    let source = std::fs::read_to_string(&arguments.file)
        .map_err(|error| format!("{}: {error}", arguments.file.display()))?;

    // Templates come from the directory the configuration is in, and from
    // nowhere else. A configuration is the tree it sits in, not the one file
    // that names it.
    let root = arguments
        .file
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);

    // The fleet is a document rather than part of the program, because the
    // machines change without the configuration changing and one configuration
    // runs against several fleets. Absent is a fleet with nothing in it, which
    // is what a configuration naming its own machines wants.
    let fleet = match &arguments.fleet {
        Some(path) => {
            let text = std::fs::read_to_string(path)
                .map_err(|error| format!("{}: {error}", path.display()))?;
            Fleet::read(&text).map_err(|problem| format!("{}: {problem}", path.display()))?
        }
        None => Fleet::empty(),
    };

    let providers = Providers::new().with(Box::new(Os::new()));

    // The providers first, so that what they refuse, such as a package reacting,
    // is refused at the line that says it.
    let graph = nerpa_config::evaluate_with(
        &arguments.file.display().to_string(),
        &source,
        Box::new(nerpa_config::template::Directory::at(root)),
        fleet.clone(),
        providers.schema(),
    )
    .map_err(|error| error.to_string())?;

    // Migrated if it is still a version 4 file, locked, and read: one step,
    // because forgetting any of the three is quiet. Planning takes the lock
    // shared, so two plans may read at once and neither reads a live run's work
    // as though that run had ended.
    let taking = match arguments.task {
        Task::Plan { .. } => shards::Taking::Reading,
        Task::Apply => shards::Taking::Writing,
    };
    let (mut state, key, _lock) =
        shards::opened(&arguments.state, taking).map_err(|error| error.to_string())?;
    let directory = shards::beside(&arguments.state);

    // Nothing in it yet: a declaration cannot say an attribute is secret until
    // the Starlark side can name a store, and the stores themselves arrive with
    // the rest of decision 0027. Wired now so that the path a secret takes
    // through a run exists rather than being retrofitted around it.
    let store = Remembered::new();
    let secrets = Secrets::new(&key, &store);

    // One connection per machine, opened before anything is read and closed
    // when the run ends. The executor is looked for beside this program: an
    // install puts them together, and a build leaves them together too.
    let machines = reachable(&graph, &fleet)?;

    let observed =
        run::observe(&graph, &providers, &machines, &state).map_err(|error| error.to_string())?;
    let plan = planner::plan(&graph, &observed, &state, &providers.schema(), secrets)
        .map_err(|error| error.to_string())?;

    if let Task::Plan { json: true } = arguments.task {
        // The same object the text is rendered from. The moment these become two
        // code paths they start disagreeing about what the plan says.
        let written = serde_json::to_string_pretty(&plan)
            .map_err(|error| format!("the plan could not be written as JSON: {error}"))?;
        println!("{written}");
        return Ok(());
    }

    print!("{}", render::plan(&plan));

    if matches!(arguments.task, Task::Plan { .. }) {
        return Ok(());
    }

    if plan.is_empty() {
        return Ok(());
    }

    // Written after every change rather than once at the end. A run that stops
    // half way — a machine that goes away, a contract violation panicking by
    // design, somebody's Ctrl-C — made real changes, and forgetting them would
    // leave them unowned for ever.
    let mut written =
        |state: &State| shards::save(&directory, state, &key).map_err(|error| error.to_string());
    let applied = run::apply(
        &plan,
        &providers,
        &machines,
        &observed,
        &mut state,
        secrets,
        &mut written,
    );

    // And once more at the end, so that a run which changed nothing still leaves
    // a state file where the next one expects it.
    shards::save(&directory, &state, &key).map_err(|error| error.to_string())?;

    let applied = applied.map_err(|error| error.to_string())?;
    println!("\napplied {} changes", applied.changed());
    Ok(())
}

// The step that decides whether one machine being down ends a run. Attacked
// through the opening it takes, which is why it takes one.
#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "tests may be blunt: see docs/CODING_STANDARDS.md"
)]
mod tests {
    use super::{Machines, Reach, Target, registry};
    use nerpa_transport::Here;

    fn node(name: &str) -> Target {
        Target::new(name).expect("a node name")
    }

    fn opens(node: &Target) -> Result<Box<dyn Reach>, String> {
        if node.as_str() == "down" {
            return Err("down: could not be reached: it is switched off".to_owned());
        }
        Ok(Box::new(Here))
    }

    #[test]
    fn a_connection_that_fails_before_anything_is_read_does_not_end_the_run() {
        let nodes = [node("up"), node("down"), node("also-up")];
        let (machines, _) = registry(&nodes, &opens);

        assert!(
            machines.reach(Some(&node("up"))).is_ok(),
            "a machine that opened is in the registry"
        );
        assert!(
            machines.reach(Some(&node("also-up"))).is_ok(),
            "and so is one that comes after the failure, which is the whole point: \
             opening happens before anything is read, so a failure here used to end \
             the run before the planner could report it"
        );
        assert!(
            machines.reach(Some(&node("down"))).is_err(),
            "a machine that would not open is left out, and asking for it refuses"
        );
    }

    #[test]
    fn the_machine_that_would_not_open_is_named_once_and_says_why() {
        let nodes = [node("up"), node("down")];
        let (_, refused) = registry(&nodes, &opens);

        assert_eq!(refused.len(), 1, "one machine refused, one complaint");
        assert!(
            refused[0].contains("down") && refused[0].contains("switched off"),
            "the complaint carries the machine and the far end's own reason, \
             rather than one invented here: {:?}",
            refused[0]
        );
    }

    #[test]
    fn opened_side_by_side_every_machine_is_accounted_for_in_the_fleets_order() {
        let nodes: Vec<Target> = (0..40)
            .map(|at| {
                if at % 3 == 0 {
                    node(&format!("down-{at}"))
                } else {
                    node(&format!("up-{at}"))
                }
            })
            .collect();
        let opening = |node: &Target| -> Result<Box<dyn Reach>, String> {
            if node.as_str().starts_with("down") {
                return Err(format!("{node}: could not be reached"));
            }
            Ok(Box::new(Here))
        };

        let (machines, refused) = registry(&nodes, &opening);

        let expected: Vec<String> = nodes
            .iter()
            .filter(|node| node.as_str().starts_with("down"))
            .map(|node| format!("{node}: could not be reached"))
            .collect();
        assert_eq!(
            refused, expected,
            "every refusal once, in the fleet's order rather than the order the \
             handshakes happened to finish in"
        );
        for up in nodes.iter().filter(|node| node.as_str().starts_with("up")) {
            assert!(machines.reach(Some(up)).is_ok(), "{up} opened and is there");
        }
    }

    #[test]
    fn a_fleet_where_nothing_opens_is_empty_rather_than_fatal() {
        let nodes = [node("down")];
        let (machines, refused) = registry(&nodes, &opens);

        assert_eq!(refused.len(), 1);
        assert!(
            machines.reach(Some(&node("down"))).is_err(),
            "no machine is invented to stand in for one that is not there"
        );
        let _: Machines = machines;
    }
}
