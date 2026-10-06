//! A package and a service all the way round the loop, and then again.
//!
//! What this provider does to a machine is mostly commands, and nothing ran
//! them. Files have a real filesystem to converge against and a machine in
//! memory to be kept off it; packages and services were checked as far as the
//! operations they build and no further, so that a second run has nothing to do
//! was a claim about them rather than a result.
//!
//! So: a machine in memory that keeps a package database and a service table,
//! answers what this provider asks and carries out what it sends. Both
//! families, because the two spell every question and every command
//! differently, and a machine that understood only one would let the other's
//! spelling rot. A command it does not know exits 127, which fails the run —
//! this cannot pass by agreeing with a provider that has changed.
//!
//! Decision 0015 asks every resource to converge twice. This is where a package
//! and a service get their second run.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use nerpa_core::address::address;

use nerpa_core::provider::{Machines, ProviderError, Providers, Reach};
use nerpa_core::run::{apply, observe};
use nerpa_core::secret::{Key, Remembered, Secrets};
use nerpa_core::state::State;
use nerpa_core::{Graph, Resource, Target, Value};
use nerpa_proto::{Answer, Ask, At, Entry, Observation, Operation, Seen};
use nerpa_provider_os::Os;

/// The plan a test drives, attributed to a tester: `run::apply` proceeds only
/// with an actor identity (`INV-TRUST-001`), and a test is a run of a kind.
fn plan(
    graph: &Graph,
    observed: &nerpa_core::Observed,
    state: &State,
    schema: &nerpa_core::Schema,
    secrets: Secrets<'_>,
) -> Result<nerpa_core::Plan, nerpa_core::planner::PlanError> {
    nerpa_core::planner::plan(graph, observed, state, schema, secrets).map(|planned| {
        planned.attributed_to(
            nerpa_core::Actor::new("tester", nerpa_core::Grant::default(), None).expect("an actor"),
        )
    })
}

/// Which package manager a machine is managed by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Family {
    Rhel,
    Portage,
    Debian,
}

/// And which init system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Init {
    Systemd,
    OpenRc,
}

/// A service, as the machine has it.
#[derive(Debug, Clone, Copy, Default)]
struct Unit {
    running: bool,
    enabled: bool,
    /// How many times it has been told to restart while running, which is what
    /// a reaction costs and the one thing an idempotent run must not repeat.
    roused: usize,
}

#[derive(Debug, Default)]
struct Inside {
    /// What the repository has to offer: atom to version.
    offered: BTreeMap<String, String>,
    /// What is installed: atom to version.
    installed: BTreeMap<String, String>,
    /// The units this machine has at all, whether running or not.
    units: BTreeMap<String, Unit>,
    /// Files, for the one case where a service reacts to one.
    paths: BTreeMap<String, Entry>,
    /// Every command that was carried out, in order.
    ran: Vec<String>,
}

/// A machine in memory that keeps packages and services.
///
/// Cloned rather than borrowed: the registry wants a machine of its own, and a
/// test wants to keep looking at the one it made. Both are the same machine,
/// because what a machine is here is what it holds.
#[derive(Debug, Clone)]
struct Machine {
    family: Family,
    init: Init,
    inside: Arc<Mutex<Inside>>,
}

impl Machine {
    fn new(family: Family, init: Init) -> Self {
        Self {
            family,
            init,
            inside: Arc::new(Mutex::new(Inside::default())),
        }
    }

    /// What its package manager could install if it were asked.
    fn offering(self, atom: &str, version: &str) -> Self {
        self.inside
            .lock()
            .unwrap()
            .offered
            .insert(atom.to_owned(), version.to_owned());
        self
    }

    /// A unit that exists on the machine, stopped and not enabled.
    fn with_unit(self, name: &str) -> Self {
        self.inside
            .lock()
            .unwrap()
            .units
            .insert(name.to_owned(), Unit::default());
        self
    }

    /// A unit that exists on the machine, in a given running and enabled state.
    ///
    /// For `OpenRC`, being enabled is a symlink into the runlevel, so that is
    /// made as well: the unit's `enabled` field alone is invisible to the read.
    fn with_unit_in(self, name: &str, running: bool, enabled: bool) -> Self {
        let init = self.init;
        {
            let mut inside = self.inside.lock().unwrap();
            inside.units.insert(
                name.to_owned(),
                Unit {
                    running,
                    enabled,
                    roused: 0,
                },
            );
            if init == Init::OpenRc && enabled {
                inside.paths.insert(
                    format!("/etc/runlevels/default/{name}"),
                    Entry::Symlink {
                        target: format!("/etc/init.d/{name}"),
                    },
                );
            }
        }
        self
    }

    fn installed(&self, atom: &str) -> Option<String> {
        self.inside.lock().unwrap().installed.get(atom).cloned()
    }

    fn unit(&self, name: &str) -> Unit {
        self.inside
            .lock()
            .unwrap()
            .units
            .get(name)
            .copied()
            .unwrap_or_default()
    }

    /// Everything it was asked to carry out, in order.
    fn ran(&self) -> Vec<String> {
        self.inside.lock().unwrap().ran.clone()
    }

    /// What this machine calls itself, which is how the provider decides how to
    /// speak to it.
    fn os_release(&self) -> Vec<u8> {
        match self.family {
            Family::Rhel => {
                b"NAME=\"Rocky Linux\"\nID=\"rocky\"\nID_LIKE=\"rhel centos fedora\"\n".to_vec()
            }
            Family::Portage => b"NAME=Gentoo\nID='gentoo'\n".to_vec(),
            Family::Debian => b"NAME=\"Debian GNU/Linux\"\nID=debian\n".to_vec(),
        }
    }
}

/// A program's answer.
fn said(status: i32, out: &str) -> Seen {
    Seen::Said {
        status,
        out: out.as_bytes().to_vec(),
        complaint: Vec::new(),
    }
}

/// A portage atom without the slot a declaration may have named.
fn stem(atom: &str) -> &str {
    atom.split(':').next().unwrap_or(atom)
}

impl Machine {
    /// What a question about this machine finds.
    fn looked(&self, inside: &Inside, observation: &Observation) -> Seen {
        match observation {
            Observation::Contents { path } if path == "/etc/os-release" => {
                Seen::Entry(Entry::File {
                    content: self.os_release(),
                    mode: 0o644,
                })
            }
            // What an init system leaves under `/run` while it is running.
            Observation::Path { path }
                if path == "/run/systemd/system" && self.init == Init::Systemd =>
            {
                Seen::Entry(Entry::Directory { mode: 0o755 })
            }
            Observation::Path { path } if path == "/run/openrc" && self.init == Init::OpenRc => {
                Seen::Entry(Entry::Directory { mode: 0o755 })
            }
            Observation::Path { path } | Observation::Contents { path } => inside
                .paths
                .get(path)
                .cloned()
                .map_or(Seen::Nothing, Seen::Entry),
            Observation::Output { program, arguments } => asked(inside, program, arguments),
            _ => Seen::Nothing,
        }
    }
}

/// What carrying out a command does to a machine, and what it says.
///
/// A batch ends by asking the same question the read asked, so anything that is
/// not a command falls through to being one.
fn ran_it(inside: &mut Inside, program: &str, arguments: &[String]) -> Seen {
    let words: Vec<&str> = arguments.iter().map(String::as_str).collect();
    match managed(inside, program, &words).or_else(|| served(inside, program, &words)) {
        Some(seen) => seen,
        None => asked(inside, program, arguments),
    }
}

/// What a program says, run by a machine that has read nothing but this
/// provider's own source to know what it might be asked.
fn asked(inside: &Inside, program: &str, arguments: &[String]) -> Seen {
    let words: Vec<&str> = arguments.iter().map(String::as_str).collect();
    match (program, words.as_slice()) {
        ("uname", ["-m"]) => said(0, "x86_64\n"),
        // `rpm` and `dpkg-query` both exit 1 for a package that is not
        // installed, and print the version on stdout.
        ("rpm", ["-q", "--queryformat", _, atom]) | ("dpkg-query", ["-W", _, atom]) => {
            match inside.installed.get(*atom) {
                Some(version) => said(0, &format!("{version}\n")),
                None => said(1, ""),
            }
        }
        // `portageq` exits 0 either way and says nothing for a package that is
        // not there; what it prints is the atom with its version.
        ("portageq", ["match", "/", atom]) => match inside.installed.get(*atom) {
            Some(version) => said(0, &format!("{}-{version}\n", stem(atom))),
            None => said(0, "\n"),
        },
        ("systemctl", ["show", name, "-p", ..]) => match inside.units.get(*name) {
            None => said(
                0,
                "LoadState=not-found\nActiveState=inactive\nUnitFileState=\n",
            ),
            Some(unit) => said(
                0,
                &format!(
                    "LoadState=loaded\nActiveState={}\nUnitFileState={}\n",
                    if unit.running { "active" } else { "inactive" },
                    if unit.enabled { "enabled" } else { "disabled" }
                ),
            ),
        },
        // `rc-service` says whether a service is running by exiting: 0 running,
        // 3 stopped, 1 no such service.
        ("rc-service", [name, "status"]) => match inside.units.get(*name) {
            Some(unit) if unit.running => said(0, "status: started\n"),
            Some(_) => said(3, "status: stopped\n"),
            None => said(1, ""),
        },
        _ => said(
            127,
            &format!("this machine has never heard of `{program}` with {words:?}"),
        ),
    }
}

/// What a package manager's commands do, or `None` for something else.
fn managed(inside: &mut Inside, program: &str, words: &[&str]) -> Option<Seen> {
    // What `install` sends is the atom and the version run together, which only
    // the repository can take apart again.
    let offered = |inside: &Inside, spelt: &dyn Fn(&str, &str) -> String, wanted: &str| {
        inside
            .offered
            .iter()
            .find(|(atom, version)| wanted == spelt(atom, version))
            .map(|(atom, version)| (atom.clone(), version.clone()))
    };
    Some(match (program, words) {
        ("dnf", ["--assumeyes", "--quiet", "install", wanted]) => {
            match offered(inside, &|atom, version| format!("{atom}-{version}"), wanted) {
                Some((atom, version)) => {
                    inside.installed.insert(atom, version);
                    said(0, "")
                }
                None => said(1, "no package matches"),
            }
        }
        ("emerge", ["--quiet", "--oneshot", wanted]) => {
            let spelt = |atom: &str, version: &str| {
                let slot = atom
                    .split_once(':')
                    .map_or(String::new(), |(_, slot)| format!(":{slot}"));
                format!("={}-{version}{slot}", stem(atom))
            };
            match offered(inside, &spelt, wanted) {
                Some((atom, version)) => {
                    inside.installed.insert(atom, version);
                    said(0, "")
                }
                None => said(1, "no ebuild matches"),
            }
        }
        ("apt-get", ["install", "-y", "-qq", wanted]) => {
            match offered(inside, &|atom, version| format!("{atom}={version}"), wanted) {
                Some((atom, version)) => {
                    inside.installed.insert(atom, version);
                    said(0, "")
                }
                None => said(1, "no package matches"),
            }
        }
        ("dnf", ["--assumeyes", "--quiet", "remove", atom])
        | ("emerge", ["--quiet", "--depclean", atom])
        | ("apt-get", ["remove", "-y", "-qq", atom]) => {
            inside.installed.remove(*atom);
            said(0, "")
        }
        _ => return None,
    })
}

/// What an init system's commands do, or `None` for something else.
fn served(inside: &mut Inside, program: &str, words: &[&str]) -> Option<Seen> {
    let mut unit = |name: &str, change: &dyn Fn(&mut Unit)| {
        if let Some(unit) = inside.units.get_mut(name) {
            change(unit);
        }
    };
    Some(match (program, words) {
        ("systemctl", ["enable", name]) => {
            unit(name, &|unit| unit.enabled = true);
            said(0, "")
        }
        ("systemctl", ["disable", name]) => {
            unit(name, &|unit| unit.enabled = false);
            said(0, "")
        }
        ("systemctl", ["start", name]) => {
            unit(name, &|unit| unit.running = true);
            said(0, "")
        }
        ("systemctl", ["stop", name]) => {
            unit(name, &|unit| unit.running = false);
            said(0, "")
        }
        // Does nothing at all to a unit that is not running, which is the whole
        // reason it is `try-restart` and not `restart`.
        ("systemctl", ["try-restart", name]) | ("rc-service", ["--ifstarted", name, "restart"]) => {
            unit(name, &|unit| {
                if unit.running {
                    unit.roused += 1;
                }
            });
            said(0, "")
        }
        // Enabled, for OpenRC, is a symlink into the runlevel and nothing else:
        // that is what the provider reads, so that is what has to be there.
        ("rc-update", ["add", name, "default"]) => {
            unit(name, &|unit| unit.enabled = true);
            inside.paths.insert(
                format!("/etc/runlevels/default/{name}"),
                Entry::Symlink {
                    target: format!("/etc/init.d/{name}"),
                },
            );
            said(0, "")
        }
        ("rc-update", ["del", name, "default"]) => {
            unit(name, &|unit| unit.enabled = false);
            inside
                .paths
                .remove(&format!("/etc/runlevels/default/{name}"));
            said(0, "")
        }
        ("rc-service", [name, "start"]) => {
            unit(name, &|unit| unit.running = true);
            said(0, "")
        }
        ("rc-service", [name, "stop"]) => {
            unit(name, &|unit| unit.running = false);
            said(0, "")
        }
        _ => return None,
    })
}

impl Reach for Machine {
    fn ask(&self, ask: &Ask) -> Result<Answer, ProviderError> {
        let mut inside = self
            .inside
            .lock()
            .map_err(|_| ProviderError::NotOnAMachine)?;
        match ask {
            Ask::Look { observations, .. } => Ok(Answer::Found {
                seen: observations
                    .iter()
                    .map(|observation| self.looked(&inside, observation))
                    .collect(),
            }),
            Ask::Perform { operations, .. } => {
                let mut became = Vec::with_capacity(operations.len());
                for operation in operations {
                    let seen = match operation {
                        Operation::Process {
                            program,
                            arguments,
                            expect,
                            ..
                        } => {
                            inside.ran.push(
                                std::iter::once(program.as_str())
                                    .chain(arguments.iter().map(String::as_str))
                                    .collect::<Vec<_>>()
                                    .join(" "),
                            );
                            let seen = ran_it(&mut inside, program, arguments);
                            let wanted: &[i32] = if expect.is_empty() { &[0] } else { expect };
                            if let Seen::Said { status, out, .. } = &seen
                                && !wanted.contains(status)
                            {
                                return Ok(Answer::Failed {
                                    at: At::Operation(became.len()),
                                    problem: format!(
                                        "{program} exited {status}, expected one of {wanted:?}: {}",
                                        String::from_utf8_lossy(out)
                                    ),
                                });
                            }
                            seen
                        }
                        Operation::File {
                            path,
                            content,
                            mode,
                            ..
                        } => {
                            let entry = Entry::File {
                                content: content.clone(),
                                mode: *mode,
                            };
                            inside.paths.insert(path.clone(), entry.clone());
                            Seen::Digest {
                                digest: nerpa_proto::digest(&Seen::Entry(entry)),
                            }
                        }
                        Operation::Directory { path, mode } => {
                            let entry = Entry::Directory { mode: *mode };
                            inside.paths.insert(path.clone(), entry.clone());
                            Seen::Entry(entry)
                        }
                        Operation::Remove { path } => {
                            inside.paths.remove(path);
                            Seen::Nothing
                        }
                        _ => {
                            return Ok(Answer::Failed {
                                at: At::Operation(became.len()),
                                problem: "this machine does not do that".to_owned(),
                            });
                        }
                    };
                    became.push(seen);
                }
                Ok(Answer::Done { became })
            }
            _ => Ok(Answer::Failed {
                at: At::Before,
                problem: "this machine does not answer that".to_owned(),
            }),
        }
    }
}

static DIGEST_KEY: Key = Key::from_bytes([11; 32]);
static NO_STORE: Remembered = Remembered::EMPTY;

fn secrets() -> Secrets<'static> {
    Secrets::new(&DIGEST_KEY, &NO_STORE)
}

#[allow(
    clippy::unnecessary_wraps,
    reason = "the shape belongs to `apply`, not to this function"
)]
fn kept(_: &State) -> Result<(), String> {
    Ok(())
}

fn graph_of(resources: Vec<Resource>) -> Graph {
    let mut graph = Graph::new();
    graph.preexisting(Target::new("web-01").unwrap());
    for resource in resources {
        graph.insert(resource).unwrap();
    }
    graph
}

/// One run of the whole loop, and how many changes it made.
fn run_once(graph: &Graph, state: &mut State, machine: &Machine) -> usize {
    let providers = Providers::new().with(Box::new(Os::new()));
    let machines = Machines::new().everywhere(Box::new(machine.clone()));
    let observed = observe(graph, &providers, &machines, state).expect("readable");
    let plan = plan(graph, &observed, state, &providers.schema(), secrets()).expect("plannable");
    apply(
        &plan,
        &providers,
        &machines,
        &observed,
        state,
        secrets(),
        &mut kept,
    )
    .expect("applies")
    .changed()
}

/// A package this provider installed is installed, and the next run agrees.
///
/// Installing is not the hard half: reading back what the machine ended up with
/// is, because both package managers report success for work they did not do.
#[test]
fn a_package_is_installed_and_the_second_run_has_nothing_to_do() {
    for (family, init, atom, version) in [
        (Family::Rhel, Init::Systemd, "nginx", "1.24.0-1"),
        (Family::Portage, Init::OpenRc, "www-servers/nginx", "1.24.0"),
        (Family::Debian, Init::Systemd, "nginx", "1.24.0-1"),
    ] {
        let machine = Machine::new(family, init).offering(atom, version);
        let graph = graph_of(vec![
            Resource::new(address(&format!("os:package.{atom}@web-01"))).with("version", version),
        ]);
        let mut state = State::new();

        assert_eq!(run_once(&graph, &mut state, &machine), 1, "{family:?}");
        assert_eq!(
            machine.installed(atom).as_deref(),
            Some(version),
            "{family:?}: it is on the machine, at the version asked for"
        );

        let ran = machine.ran().len();
        assert_eq!(
            run_once(&graph, &mut state, &machine),
            0,
            "{family:?}: the second run has nothing to do"
        );
        assert_eq!(
            machine.ran().len(),
            ran,
            "{family:?}: and sends nothing to the machine: {:?}",
            machine.ran()
        );
    }
}

/// A package whose declaration goes is taken off the machine.
#[test]
fn a_package_whose_declaration_goes_is_removed() {
    for (family, init, atom, version) in [
        (Family::Rhel, Init::Systemd, "nginx", "1.24.0-1"),
        (Family::Portage, Init::OpenRc, "www-servers/nginx", "1.24.0"),
        (Family::Debian, Init::Systemd, "nginx", "1.24.0-1"),
    ] {
        let machine = Machine::new(family, init).offering(atom, version);
        let declared = graph_of(vec![
            Resource::new(address(&format!("os:package.{atom}@web-01"))).with("version", version),
        ]);
        let mut state = State::new();
        run_once(&declared, &mut state, &machine);

        assert_eq!(run_once(&graph_of(Vec::new()), &mut state, &machine), 1);
        assert_eq!(
            machine.installed(atom),
            None,
            "{family:?}: it is gone from the machine"
        );
        assert!(
            state.is_empty(),
            "{family:?}: and gone from state: {:?}",
            state
                .addresses()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        );
    }
}

/// A service asked to run runs, is enabled, and the next run leaves it alone.
#[test]
fn a_service_is_settled_and_the_second_run_has_nothing_to_do() {
    for (family, init) in [
        (Family::Rhel, Init::Systemd),
        (Family::Portage, Init::OpenRc),
        (Family::Debian, Init::Systemd),
    ] {
        let machine = Machine::new(family, init).with_unit("nginx");
        let graph = graph_of(vec![
            Resource::new(address("os:service.nginx@web-01"))
                .with("running", Value::Boolean(true))
                .with("enabled", Value::Boolean(true)),
        ]);
        let mut state = State::new();

        assert_eq!(run_once(&graph, &mut state, &machine), 1, "{init:?}");
        let unit = machine.unit("nginx");
        assert!(unit.running && unit.enabled, "{init:?}: {unit:?}");

        let ran = machine.ran().len();
        assert_eq!(
            run_once(&graph, &mut state, &machine),
            0,
            "{init:?}: the second run has nothing to do"
        );
        assert_eq!(
            machine.ran().len(),
            ran,
            "{init:?}: and sends nothing: {:?}",
            machine.ran()
        );
    }
}

/// A running service takes a changed file once, and is left alone after that.
///
/// The count is the point. A restart is the one thing here that is felt by
/// whoever is using the service, and a run that repeats it because the last one
/// did not write down that it happened is the failure decision 0026 is about.
#[test]
fn a_running_service_takes_a_changed_file_once() {
    for (family, init) in [
        (Family::Rhel, Init::Systemd),
        (Family::Portage, Init::OpenRc),
        (Family::Debian, Init::Systemd),
    ] {
        let machine = Machine::new(family, init).with_unit("nginx");
        let declared = |content: &str| {
            graph_of(vec![
                Resource::new(address("os:file.conf@web-01"))
                    .with("path", "/etc/nginx.conf")
                    .with("content", content),
                Resource::new(address("os:service.nginx@web-01"))
                    .with("running", Value::Boolean(true))
                    .with("enabled", Value::Boolean(true))
                    .reacting_to(address("os:file.conf@web-01")),
            ])
        };
        let mut state = State::new();
        run_once(&declared("one\n"), &mut state, &machine);
        // Settled and roused in the same run: started, and then told to take
        // the file it was handed on the way up. That is `INV-REACT-004` — a
        // service whose own attributes change in the same plan as its file is
        // one change and still has to be restarted — and for a service that was
        // not running a moment ago it is a restart nobody needed. Counted
        // rather than waved at, because it is the provider's to improve.
        assert_eq!(
            machine.unit("nginx").roused,
            1,
            "{init:?}: settled and roused at once: {:?}",
            machine.ran()
        );

        // The file it reads changes.
        assert_eq!(
            run_once(&declared("two\n"), &mut state, &machine),
            2,
            "{init:?}"
        );
        assert_eq!(
            machine.unit("nginx").roused,
            2,
            "{init:?}: it took the new file, once"
        );

        assert_eq!(
            run_once(&declared("two\n"), &mut state, &machine),
            0,
            "{init:?}: and the run after that has nothing to do"
        );
        assert_eq!(
            machine.unit("nginx").roused,
            2,
            "{init:?}: nobody is restarted twice for one change: {:?}",
            machine.ran()
        );
    }
}

/// A service declared without `enabled` or `running` is not disabled or stopped.
///
/// The half that is declared is settled; the other, never named, is left exactly
/// as the machine had it. Declaring only `running` used to disable the service on
/// its way to starting it.
#[test]
fn a_service_declared_without_enabled_or_running_is_not_disabled_or_stopped() {
    for (family, init) in [
        (Family::Rhel, Init::Systemd),
        (Family::Portage, Init::OpenRc),
        (Family::Debian, Init::Systemd),
    ] {
        // Stopped but enabled, then declared only as running: `enabled` is never
        // named and must stay as it was.
        let machine = Machine::new(family, init).with_unit_in("nginx", false, true);
        let graph = graph_of(vec![
            Resource::new(address("os:service.nginx@web-01")).with("running", Value::Boolean(true)),
        ]);
        let mut state = State::new();

        assert_eq!(run_once(&graph, &mut state, &machine), 1, "{init:?}");
        let unit = machine.unit("nginx");
        assert!(unit.running, "{init:?}: it was started");
        assert!(
            unit.enabled,
            "{init:?}: `enabled` was never declared and is not disabled: {:?}",
            machine.ran()
        );

        assert_eq!(
            run_once(&graph, &mut state, &machine),
            0,
            "{init:?}: and the run after that has nothing to do"
        );
    }
}

/// A service whose declaration is removed is left running and enabled.
///
/// Removing the declaration removes the declaration, not the unit: the unit came
/// from a package and stays, so nothing is stopped or disabled. It used to settle
/// the unit to stopped and disabled on the way out.
#[test]
fn a_removed_service_declaration_stops_or_disables_nothing() {
    for (family, init) in [
        (Family::Rhel, Init::Systemd),
        (Family::Portage, Init::OpenRc),
        (Family::Debian, Init::Systemd),
    ] {
        let machine = Machine::new(family, init).with_unit("nginx");
        let declared = graph_of(vec![
            Resource::new(address("os:service.nginx@web-01"))
                .with("running", Value::Boolean(true))
                .with("enabled", Value::Boolean(true)),
        ]);
        let mut state = State::new();
        assert_eq!(run_once(&declared, &mut state, &machine), 1, "{init:?}");

        // The declaration goes; the unit stays running and enabled.
        assert_eq!(
            run_once(&graph_of(Vec::new()), &mut state, &machine),
            1,
            "{init:?}"
        );
        let unit = machine.unit("nginx");
        assert!(
            unit.running && unit.enabled,
            "{init:?}: the unit is neither stopped nor disabled: {:?}",
            machine.ran()
        );
        assert!(
            state.is_empty(),
            "{init:?}: and the declaration is gone from state: {:?}",
            state
                .addresses()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        );
    }
}
