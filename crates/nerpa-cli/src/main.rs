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

mod approval;
mod journal;
mod render;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use nerpa_config::fleet::{Fleet, Route};
use nerpa_core::address::{Address, Target};
use nerpa_core::plan::{ChangeKind, Plan};
use nerpa_core::provider::{Machines, Reach};
use nerpa_core::secret::{Remembered, Secrets};
use nerpa_core::shards;
use nerpa_core::state::{Released, State};
use nerpa_core::{Actor, Grant, Inventory, Provenance};
use nerpa_engine::Engine;

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

    /// Who is acting in this run — a person, CI, or an agent.
    ///
    /// A run without an actor does not start (`INV-TRUST-001`).
    #[arg(long = "as", value_name = "WHO", global = true)]
    actor: Option<String>,

    /// The grant that limits what this actor may touch: kinds, targets and
    /// operations, written `kind=a,b;target=c;op=d,e` (adr-2026-009).
    #[arg(
        long = "as-scope",
        value_name = "GRANT",
        default_value = "all",
        global = true
    )]
    actor_scope: String,

    /// When this identity stops being valid, RFC 3339. Carried now; a grant
    /// checks it later.
    #[arg(long = "as-expiry", value_name = "RFC3339", global = true)]
    actor_expiry: Option<String>,
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
    Apply {
        /// The approval over this plan: somebody else's detached signature
        /// over its number (`0009`). Where the fleet names who may sign, this
        /// is required.
        #[arg(long, value_name = "FILE")]
        approval: Option<PathBuf>,
    },
    /// Say a machine is yours again after a run was killed while changing it.
    ///
    /// Nothing here can tell whether those changes arrived; somebody who can
    /// look at the machine can. What was in flight becomes what the next plan
    /// has to deal with rather than being thrown away.
    Release {
        /// Which machine.
        machine: Option<String>,
        /// What lives on no machine, which has no name a machine could share.
        #[arg(long, conflicts_with = "machine")]
        control: bool,
        /// One entry, by address: you looked, and there is nothing there.
        ///
        /// What the entry remembers will not be removed when its declaration
        /// goes, because nothing remembers it any more.
        #[arg(long, conflicts_with_all = ["machine", "control"], value_name = "ADDRESS")]
        entry: Option<String>,
    },
    /// Sign a plan's number, as the third party whose approval an apply needs.
    ///
    /// The signer is somebody other than whoever proposed the plan (`0009`).
    /// The key is handed to `ssh-keygen` rather than read here: it may be a
    /// private key file, or a public one whose other half is in `ssh-agent`.
    Sign {
        /// The plan to approve, as `nerpa plan --json` wrote it.
        plan: PathBuf,
        /// The key to sign with.
        #[arg(long = "with-key", value_name = "KEY")]
        with_key: PathBuf,
        /// Where the approval is written. Beside the plan, named after it.
        #[arg(long, value_name = "FILE")]
        out: Option<PathBuf>,
    },
    /// The run journal beside state, for a person reading afterwards.
    Journal {
        #[command(subcommand)]
        what: JournalTask,
    },
    /// Read a Terraform state and write the configuration and state that
    /// declare what it made. A migration, not a run: nothing here reaches the
    /// cloud, and the state is Terraform's, on the way out.
    Import {
        /// The `terraform.tfstate` to read.
        tfstate: PathBuf,
    },
}

/// What to do with the run journal.
#[derive(Subcommand)]
enum JournalTask {
    /// Say whether the log verifies from the first entry.
    ///
    /// A read of the record and nothing else: nothing is appended, because a
    /// verification that wrote to the log it is reading would be verifying
    /// something it had just changed.
    Verify,
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
    let inventory = graph.inventory();
    let (machines, refused) = registry(&nodes, &|node| {
        speaking(node, fleet, inventory, &beside, &carried)
    });
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

#[dacc_derive::doc_anchor(id = "inv-reach-001-1")]
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
#[dacc_derive::doc_anchor(id = "inv-wire-003-1")]
fn speaking(
    node: &Target,
    fleet: &Fleet,
    inventory: &Inventory,
    beside: &Path,
    carried: &Result<Carried, String>,
) -> Result<Box<dyn Reach>, String> {
    // Refused rather than reached here: a node a resource creates is a machine
    // other than this one, and `route` answers `Here` for any node the fleet does
    // not describe. Only a route the fleet names says where it is.
    if let Some(Provenance::Managed(creator)) = inventory.provenance(node)
        && !fleet.describes(node)
    {
        return Err(format!(
            "{node}: created by {creator}, and the fleet document names no route to it"
        ));
    }
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
    // Which build answered is asked before anything is read, because what is
    // read is the executor's account of the machine, and an account from a build
    // this nerpa does not ship is not one it has a reason to believe. Opening
    // and that question are one step, because they are the part that can be
    // started over.
    let reach = nerpa_transport::opened(node, &program, &arguments, expected)
        .map_err(|error| error.to_string())?;
    Ok(Box::new(reach))
}

/// Settles what a killed run left in flight on one machine.
///
/// Takes the lock every command that writes the directory takes, so it refuses
/// beside a live run rather than settling marks that run is about to answer for.
/// Clears one entry, which is a person saying they have looked at the machine.
///
/// The one way out that is not a run's own doing, and the reason it exists:
/// what an entry remembers is a change reading cannot settle, so somebody who
/// can see the machine is the only one who can say there is nothing there.
fn release_entry(arguments: &Arguments, entry: &str) -> Result<(), String> {
    let address: Address = entry.parse().map_err(|error| format!("{entry}: {error}"))?;

    let (mut state, key, _lock) = shards::opened(&arguments.state, shards::Taking::Writing)
        .map_err(|error| error.to_string())?;
    let said = match state.release_entry(&address) {
        Released::Nothing => format!("{address} has no entry"),
        // Said as the smaller step it asks for, not as a refusal to help: the
        // machine is held, and releasing it is what turns what is in flight
        // into the entry this would then clear.
        Released::InFlight => {
            return Err(format!(
                "{address} is still in flight; release its machine first, which settles what \
                 was in flight into an entry"
            ));
        }
        Released::Cleared(mark) => format!(
            "{address}: the entry is cleared; if that {} left anything on the machine, it is \
             not Nerpa's to remove any more",
            said_kind(mark.kind())
        ),
    };
    shards::save(&shards::beside(&arguments.state), &state, &key)
        .map_err(|error| error.to_string())?;
    println!("{said}");
    Ok(())
}

/// What a change of this kind is called, in a sentence.
fn said_kind(kind: ChangeKind) -> &'static str {
    match kind {
        ChangeKind::Create => "creation",
        ChangeKind::Update => "update",
        ChangeKind::Replace => "replacement",
        ChangeKind::Delete => "removal",
        ChangeKind::React => "restart",
        // Neither is ever marked — they send nothing — and a kind this build
        // does not know is named no better than by what it is.
        _ => "change",
    }
}

fn release(arguments: &Arguments, machine: Option<&str>, control: bool) -> Result<(), String> {
    let domain = match (machine, control) {
        (Some(name), false) => Some(Target::new(name).map_err(|error| format!("{name}: {error}"))?),
        (None, true) => None,
        (None, false) => {
            return Err("name a machine, or --control for what lives on no machine".to_owned());
        }
        (Some(_), true) => {
            return Err("name a machine or say --control, not both".to_owned());
        }
    };

    let (mut state, key, _lock) = shards::opened(&arguments.state, shards::Taking::Writing)
        .map_err(|error| error.to_string())?;
    let settled = state.release(domain.as_ref());
    shards::save(&shards::beside(&arguments.state), &state, &key)
        .map_err(|error| error.to_string())?;

    let named = domain.map_or_else(
        || "the control domain".to_owned(),
        |domain| domain.to_string(),
    );
    let said = match settled {
        0 => format!("nothing was in flight on {named}"),
        1 => format!("{named} is yours again; one change that was in flight is now an entry"),
        many => {
            format!("{named} is yours again; {many} changes that were in flight are now entries")
        }
    };
    println!("{said}");
    Ok(())
}

/// Reads a Terraform state and writes the configuration and state that declare
/// what it made. A migration, not a run: nothing here reaches the cloud.
fn imported(tfstate: &Path, config: &Path, state_path: &Path) -> Result<(), String> {
    let text = std::fs::read_to_string(tfstate)
        .map_err(|error| format!("{}: {error}", tfstate.display()))?;
    let tf: nerpa_import::TfState =
        serde_json::from_str(&text).map_err(|error| format!("{}: {error}", tfstate.display()))?;
    let import = nerpa_import::Import::read(&tf)?;

    // The secrets first: the one thing a migration must not swallow.
    for secret in &import.secrets {
        eprintln!(
            "secret in clear text: {} — {}",
            secret.resource, secret.attribute
        );
    }

    let rendered = import.render();
    std::fs::write(config, rendered).map_err(|error| format!("{}: {error}", config.display()))?;

    let key = nerpa_core::secret::Key::fresh()
        .map_err(|error| format!("no randomness for a digest key: {error}"))?;
    let recorded = import.state()?;
    nerpa_core::store::save(state_path, &recorded, &key)
        .map_err(|error| format!("{}: {error}", state_path.display()))?;

    println!(
        "imported {} resources into {} and {}",
        import.resources.len(),
        config.display(),
        state_path.display()
    );
    Ok(())
}

#[allow(
    clippy::too_many_lines,
    reason = "the run's steps stay in one place and in order: identity, release, \
              read, plan, apply, journal"
)]
fn run_task(arguments: &Arguments) -> Result<(), String> {
    // Importing is a migration, not a run: it reads a Terraform state and
    // writes a configuration and a state, and reaches no cloud.
    if let Task::Import { tfstate } = &arguments.task {
        return imported(tfstate, &arguments.file, &arguments.state);
    }

    // The identity first: a run without one does not start (`INV-TRUST-001`).
    // Refused here rather than by clap, because a global argument cannot be
    // required and the refusal has to name the flag anyway.
    let who = arguments.actor.as_deref().ok_or_else(no_actor_refused)?;
    let grant = arguments.actor_scope.parse::<Grant>()?;
    let actor = Actor::new(who, grant, arguments.actor_expiry.clone())
        .map_err(|error| error.to_string())?;

    // Before the configuration is read, because releasing a machine is about
    // what a run left behind rather than about what anybody declares.
    if let Task::Release {
        machine,
        control,
        entry,
    } = &arguments.task
    {
        let released = match entry {
            Some(entry) => release_entry(arguments, entry),
            None => release(arguments, machine.as_deref(), *control),
        };
        if released.is_ok() {
            note(&arguments.state, &format!("release as {actor}"));
        }
        return released;
    }

    // The log's own question, answered before the configuration is read:
    // verifying is about the record of what happened, not about what anybody
    // declares.
    if let Task::Journal {
        what: JournalTask::Verify,
    } = &arguments.task
    {
        return journal_verified(&arguments.state);
    }

    // Signing approves a plan that already exists: no configuration, no state,
    // no machines. The signer is the third party of `0009`, and their key goes
    // to ssh-keygen rather than being read here (`0067`).
    if let Task::Sign {
        plan,
        with_key,
        out,
    } = &arguments.task
    {
        return signed_with(plan, with_key, out.as_deref(), &actor);
    }

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

    // A machine's grant is what the fleet issued it (0072), not what it types.
    // The actor was made above over `--as-scope`, which release and signing
    // need no more of; here, where a plan is about to be made, the issued grant
    // replaces it, and a second grant typed beside it is refused.
    let actor = match fleet.grant(who) {
        Some(issued) => {
            if arguments.actor_scope != "all" {
                return Err(format!(
                    "{who} has a grant in the fleet, and `--as-scope` names what was \
                     issued: a machine's grant is issued, not typed (0072)"
                ));
            }
            Actor::new(who, issued.clone(), arguments.actor_expiry.clone())
                .map_err(|error| error.to_string())?
        }
        None => actor,
    };

    // The engine registers the providers and evaluates the configuration, so
    // what a provider refuses is refused at the line that says it.
    let engine = Engine::assemble(
        &arguments.file.display().to_string(),
        &source,
        &root,
        &fleet,
    )
    .map_err(|error| error.to_string())?;

    // Migrated if it is still a version 4 file, locked, and read: one step,
    // because forgetting any of the three is quiet. Planning takes the lock
    // shared, so two plans may read at once and neither reads a live run's work
    // as though that run had ended.
    let taking = match arguments.task {
        // The journal's question and signing are answered above and never get
        // this far; were they here, they read.
        Task::Plan { .. } | Task::Journal { .. } | Task::Sign { .. } => shards::Taking::Reading,
        // Releasing and importing return above; these only make the match total.
        Task::Apply { .. } | Task::Release { .. } | Task::Import { .. } => shards::Taking::Writing,
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
    let machines = reachable(engine.graph(), &fleet)?;

    let observed = engine
        .observe(&machines, &state)
        .map_err(|error| error.to_string())?;
    let plan = engine
        .plan(&observed, &state, secrets, actor.clone())
        .map_err(|error| error.to_string())?;

    // Where the fleet names who may sign, an apply runs only over a plan a
    // third party has signed (`0009`). Checked before the plan is printed and
    // long before anything is applied: nothing is said over a plan whose
    // approval does not hold.
    let signed_by = match (&arguments.task, fleet.approvers()) {
        (
            Task::Apply {
                approval: Some(file),
            },
            Some(approvers),
        ) => {
            let held = approval::read(file)?;
            Some(approval::checked(Path::new(approvers), &held, &plan)?)
        }
        (Task::Apply { .. }, Some(_)) => {
            return Err(
                "this fleet names who may sign, so apply needs --approval <FILE>: a \
                 plan that a third party has not signed is not applied here (0009)"
                    .to_owned(),
            );
        }
        (
            Task::Apply {
                approval: Some(file),
            },
            None,
        ) => {
            return Err(format!(
                "--approval {} holds an approval and nothing here asks for one: say \
                 who may sign in the fleet's [approval], or apply without one",
                file.display()
            ));
        }
        _ => None,
    };

    if let Task::Plan { json: true } = arguments.task {
        // The same object the text is rendered from. The moment these become two
        // code paths they start disagreeing about what the plan says.
        let written = serde_json::to_string_pretty(&plan)
            .map_err(|error| format!("the plan could not be written as JSON: {error}"))?;
        println!("{written}");
        note(
            &arguments.state,
            &format!("plan {} as {actor}", what_number(&plan)),
        );
        return Ok(());
    }

    print!("{}", render::plan(&plan));

    if matches!(arguments.task, Task::Plan { .. }) {
        note(
            &arguments.state,
            &format!("plan {} as {actor}", what_number(&plan)),
        );
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
    let applied = engine.apply(
        &plan,
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
    note(
        &arguments.state,
        &format!(
            "apply {} · {} changes as {actor}{}",
            what_number(&plan),
            applied.changed(),
            signed_by.map_or_else(String::new, |signer| format!(", signed by {signer}"))
        ),
    );
    Ok(())
}

/// Signs a plan's number with the signer's key and writes the approval beside
/// the plan, named after it.
fn signed_with(
    plan: &Path,
    with_key: &Path,
    out: Option<&Path>,
    actor: &Actor,
) -> Result<(), String> {
    let text =
        std::fs::read_to_string(plan).map_err(|error| format!("{}: {error}", plan.display()))?;
    let approved: Plan = serde_json::from_str(&text).map_err(|error| {
        format!(
            "{}: this is not a plan this build can read: {error}",
            plan.display()
        )
    })?;
    let hold = approval::approved(actor.who(), with_key, &approved)?;
    let mut named = plan.as_os_str().to_owned();
    named.push(".approval");
    let written = out.map_or_else(|| PathBuf::from(named), Path::to_path_buf);
    approval::write(&written, &hold)?;
    println!(
        "signed {} as {}: {}",
        what_number(&approved),
        actor.who(),
        written.display()
    );
    Ok(())
}

/// The refusal `INV-TRUST-001` is made of: a run without an actor identity
/// does not start, and the refusal names the flag that fixes it.
#[dacc_derive::doc_anchor(id = "inv-trust-001")]
fn no_actor_refused() -> String {
    "a run needs an actor identity: pass `--as <WHO>` (INV-TRUST-001)".to_owned()
}

/// The plan's number, or the reason it has none, as the journal writes it.
fn what_number(plan: &Plan) -> String {
    plan.digest().map_or_else(
        |problem| format!("no number: {problem}"),
        |digest| digest.to_string(),
    )
}

/// Writes the run's line to the journal beside state. A journal that cannot be
/// written says so out loud rather than failing a run that already happened.
fn note(state: &Path, line: &str) {
    if let Err(problem) = journal::record(state, line) {
        eprintln!("nerpa: the run journal could not be written: {problem}");
    }
}

/// Says whether the log beside state verifies from the first entry. The count
/// is the answer as much as the word is: a log of thirty entries that verifies
/// and one of two are different claims about the same record.
fn journal_verified(state: &Path) -> Result<(), String> {
    let entries = journal::verify(state)?;
    if entries == 0 {
        println!("the log verifies from the first entry: it holds nothing yet");
    } else {
        println!("the log verifies from the first entry: {entries} entries");
    }
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
    use super::{Machines, Reach, Target, registry, speaking};
    use nerpa_config::fleet::Fleet;
    use nerpa_core::Inventory;
    use nerpa_core::address::address;
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
    #[dacc_derive::doc_anchor(
        id = "a-connection-that-fails-before-anything-is-read-does-not-end-the-run"
    )]
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
    #[dacc_derive::doc_anchor(id = "the-machine-that-would-not-open-is-named-once-and-says-why")]
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
    #[dacc_derive::doc_anchor(id = "a-fleet-where-nothing-opens-is-empty-rather-than-fatal")]
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

    #[test]
    fn a_node_a_resource_creates_is_refused_when_the_fleet_names_no_route_to_it() {
        let mut inventory = Inventory::new();
        inventory.managed_by(node("db"), address("yandex:instance.db"));
        inventory.preexisting(node("localhost"));
        let nodes = [node("db")];
        let beside = std::path::Path::new("nerpa-executor");
        let carried = Err("not looked for".to_owned());

        let (machines, refused) = registry(&nodes, &|node| {
            speaking(node, &Fleet::empty(), &inventory, beside, &carried)
        });

        assert_eq!(refused.len(), 1, "one machine refused, one complaint");
        assert!(
            refused[0].contains("db")
                && refused[0].contains("yandex:instance.db")
                && refused[0].contains("the fleet document names no route to it"),
            "the complaint names the node, the resource that creates it, and what \
             the fleet lacks: {:?}",
            refused[0]
        );
        assert!(
            machines.reach(Some(&node("db"))).is_err(),
            "the node is not reached on the machine nerpa runs on"
        );
    }
}
