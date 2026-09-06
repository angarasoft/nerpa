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
use nerpa_core::state::State;
use nerpa_core::store::{self, Lock};
use nerpa_provider_os::Os;
use nerpa_transport::Piped;

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

/// The executor built to run somewhere that is not here.
///
/// A separate file from the one used locally, because a machine reached over
/// `ssh` is not this machine: the local build links against what is here, and a
/// static one is what travels.
fn portable(beside: &Path) -> Result<PathBuf, String> {
    let carried = beside.with_file_name("nerpa-executor.static");
    if carried.exists() {
        return Ok(carried);
    }
    Err(format!(
        "{} is not there. A machine reached over ssh needs an executor that runs \
         where it is going, and nothing names one for this node",
        carried.display()
    ))
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

    let mut machines = Machines::new();
    for node in graph.inventory().nodes() {
        // ENFORCES: INV-REACH-001
        // A machine that does not answer is left out of the registry, and the
        // planner reports it as out of reach. Every connection is opened before
        // anything is read, so a failure here happens before the planner could
        // say so on its own — and one machine switched off must not stop a
        // fleet. See decision 0020.
        match speaking(node, fleet, &beside) {
            Ok(reach) => machines = machines.with(node.clone(), reach),
            Err(problem) => eprintln!("nerpa: {problem}"),
        }
    }
    Ok(machines)
}

/// Opens the connection to one machine.
///
/// Everything that can fail because that machine is not there fails here, so
/// the caller has one place to decide what an absent machine means.
fn speaking(node: &Target, fleet: &Fleet, beside: &Path) -> Result<Box<dyn Reach>, String> {
    let (program, arguments) = match fleet.route(node) {
        Route::Here => (beside.display().to_string(), Vec::new()),
        Route::Ssh {
            host,
            executor,
            escalate,
        } => {
            // Named by the fleet, or put there. A path somebody wrote down is
            // used as written: they know something about that machine that this
            // does not.
            let executor = if let Some(named) = executor {
                named
            } else {
                let carried = portable(beside)?;
                nerpa_transport::delivery::deliver(node, &host, &carried, PORTABLE_ARCHITECTURE)
                    .map_err(|error| error.to_string())?
            };

            // `BatchMode` because a run that hangs waiting for a prompt nobody
            // will see is worse than one that fails: a fleet stops half way and
            // nothing says why. `ConnectTimeout` for the same reason, against a
            // machine that is switched off rather than one that asks a question.
            // Everything else about how this host is reached — the port, the
            // jump host, the key — belongs to ssh's own configuration and is
            // deliberately not repeated here.
            let mut arguments = vec![
                "-o".to_owned(),
                "BatchMode=yes".to_owned(),
                "-o".to_owned(),
                format!("ConnectTimeout={}", nerpa_transport::ANSWERS_WITHIN),
                host,
            ];
            arguments.extend(escalate);
            arguments.push(executor);
            ("ssh".to_owned(), arguments)
        }
    };
    let reach = Piped::running(node, &program, &arguments).map_err(|error| error.to_string())?;
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

    let graph = nerpa_config::evaluate(
        &arguments.file.display().to_string(),
        &source,
        Box::new(nerpa_config::template::Directory::at(root)),
        fleet.clone(),
    )
    .map_err(|error| error.to_string())?;

    let providers = Providers::new().with(Box::new(Os::new()));

    // Taken before anything is read, and held until the run is over. Two runs
    // against one state file would each write what the other did not know about.
    let _lock = Lock::take(&arguments.state).map_err(|error| error.to_string())?;

    let (mut state, key) = store::load(&arguments.state).map_err(|error| error.to_string())?;

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
    let mut written = |state: &State| {
        store::save(&arguments.state, state, &key).map_err(|error| error.to_string())
    };
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
    store::save(&arguments.state, &state, &key).map_err(|error| error.to_string())?;

    let applied = applied.map_err(|error| error.to_string())?;
    println!("\napplied {} changes", applied.changed());
    Ok(())
}
