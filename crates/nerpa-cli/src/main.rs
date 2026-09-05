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
use nerpa_core::planner;
use nerpa_core::provider::Providers;
use nerpa_core::run;
use nerpa_core::secret::{Remembered, Secrets};
use nerpa_core::state::State;
use nerpa_core::store::{self, Lock};
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

    let graph = nerpa_config::evaluate(
        &arguments.file.display().to_string(),
        &source,
        Box::new(nerpa_config::template::Directory::at(root)),
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

    let observed = run::observe(&graph, &providers, &state).map_err(|error| error.to_string())?;
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
