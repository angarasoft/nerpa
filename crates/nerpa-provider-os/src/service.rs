//! What a service is, per init system, and nothing else.
//!
//! The same shape as [`crate::package`] and for the same reason: every
//! difference between systems that this provider knows about lives in one file,
//! where they can be read side by side.
//!
//! # Why this needs a fact at all
//!
//! Provider logic runs where the engine runs (`0032`), so the question has to be
//! formed before anything is sent — which means knowing the init system in
//! advance. Every predecessor avoids the problem by running its own code on the
//! machine: Ansible's module, Puppet's `confine`, Salt's `virtual()` all decide
//! on the node, where the answer is already lying about. Terraform, the one
//! other tool whose logic is central, declines the problem — it has no service
//! resource.
//!
//! What is bought by asking first is that the plan can name the command before
//! it runs. A tool that decides on the machine cannot show you that.
//!
//! # Measured, not assumed
//!
//! Every command and every exit code here was run against a real machine of each
//! kind before it was written down.
//!
//! * `OpenRC` answers `rc-service <name> status` with **0** for a running service,
//!   **3** for a stopped one and **1** for one that does not exist. Three codes,
//!   three answers, no parsing of prose.
//! * `systemctl is-active <name>` is the obvious instrument and the wrong one:
//!   for a service that does not exist it prints `inactive` and exits 3, exactly
//!   as it does for one that is merely stopped. `systemctl show` distinguishes
//!   them with `LoadState=not-found`, always exits 0, and answers whether the
//!   service starts at boot in the same call.
//! * `OpenRC` has no such call: being enabled *is* a symlink at
//!   `/etc/runlevels/default/<name>`, so that is an ordinary look at a path.

use nerpa_core::value::Value;
use nerpa_proto::{Entry, Observation, Operation, Seen};

/// An init system, as far as this provider knows one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Init {
    /// `systemctl` to look and to act.
    Systemd,
    /// `rc-service` to act, and a symlink in a runlevel to be enabled.
    OpenRc,
}

/// Which runlevel counts as "at boot" for `OpenRC`.
///
/// `default` rather than `boot`: `boot` is for what the system itself needs
/// before anything else, and putting somebody's web server there would start it
/// earlier than its dependencies.
const RUNLEVEL: &str = "default";

impl Init {
    /// What a machine calling itself this is managed by, if anything here knows.
    pub(crate) fn of(said: &str) -> Option<Self> {
        match said {
            "systemd" => Some(Self::Systemd),
            "openrc" => Some(Self::OpenRc),
            _ => None,
        }
    }

    /// What to look at to find out what a service is doing.
    ///
    /// One observation for systemd, two for `OpenRC`, because `OpenRC` keeps the two
    /// answers in two places. The caller hands back exactly as many as were
    /// asked for.
    pub(crate) fn look(self, name: &str) -> Vec<Observation> {
        match self {
            Self::Systemd => vec![Observation::Output {
                program: "systemctl".to_owned(),
                arguments: vec![
                    "show".to_owned(),
                    name.to_owned(),
                    "-p".to_owned(),
                    "LoadState".to_owned(),
                    "-p".to_owned(),
                    "ActiveState".to_owned(),
                    "-p".to_owned(),
                    "UnitFileState".to_owned(),
                ],
            }],
            Self::OpenRc => vec![
                Observation::Output {
                    program: "rc-service".to_owned(),
                    arguments: vec![name.to_owned(), "status".to_owned()],
                },
                Observation::Path {
                    path: format!("/etc/runlevels/{RUNLEVEL}/{name}"),
                },
            ],
        }
    }

    /// What the answers mean: running and enabled, or `None` for no such
    /// service.
    ///
    /// A service that is not installed is absent, which is an answer. A machine
    /// that would not say is an error, because guessing "stopped" would have
    /// nerpa try to start something that is not there.
    pub(crate) fn understood(
        self,
        name: &str,
        seen: &[Seen],
    ) -> Result<Option<(bool, bool)>, String> {
        match self {
            Self::Systemd => {
                let Some(Seen::Said { status: 0, out, .. }) = seen.first() else {
                    return Err(format!("{name}: systemctl did not answer"));
                };
                let said = String::from_utf8_lossy(out);
                let field = |key: &str| {
                    said.lines()
                        .find_map(|line| line.strip_prefix(key))
                        .map(str::trim)
                };
                if field("LoadState=") == Some("not-found") {
                    return Ok(None);
                }
                Ok(Some((
                    field("ActiveState=") == Some("active"),
                    field("UnitFileState=") == Some("enabled"),
                )))
            }
            Self::OpenRc => {
                let Some(Seen::Said { status, .. }) = seen.first() else {
                    return Err(format!("{name}: rc-service did not answer"));
                };
                let running = match status {
                    0 => true,
                    3 => false,
                    // 1 is `service 'x' does not exist`, measured.
                    1 => return Ok(None),
                    other => return Err(format!("{name}: rc-service exited {other}")),
                };
                // Enabled is a symlink into the runlevel, so it is there or it
                // is not. A file rather than a link would be somebody's doing
                // and is not an answer this knows how to read.
                let enabled = matches!(seen.get(1), Some(Seen::Entry(Entry::Symlink { .. })));
                Ok(Some((running, enabled)))
            }
        }
    }

    /// What to run to make a service match what was declared.
    ///
    /// Both halves in one batch: a service that should be running and starting
    /// at boot is two changes to a machine, and a batch that made one of them
    /// and stopped would leave a state nobody asked for.
    pub(crate) fn settle(self, name: &str, running: bool, enabled: bool) -> Vec<Operation> {
        let run = |program: &str, arguments: Vec<String>| Operation::Process {
            program: program.to_owned(),
            arguments,
            stdin: Vec::new(),
            private: false,
            expect: vec![0],
        };
        match self {
            Self::Systemd => vec![
                run(
                    "systemctl",
                    vec![
                        if enabled { "enable" } else { "disable" }.to_owned(),
                        name.to_owned(),
                    ],
                ),
                run(
                    "systemctl",
                    vec![
                        if running { "start" } else { "stop" }.to_owned(),
                        name.to_owned(),
                    ],
                ),
            ],
            Self::OpenRc => vec![
                run(
                    "rc-update",
                    vec![
                        if enabled { "add" } else { "del" }.to_owned(),
                        name.to_owned(),
                        RUNLEVEL.to_owned(),
                    ],
                ),
                run(
                    "rc-service",
                    vec![
                        name.to_owned(),
                        if running { "start" } else { "stop" }.to_owned(),
                    ],
                ),
            ],
        }
    }

    // ENFORCES: INV-REACT-003
    /// What to run to make a running service take a changed file.
    ///
    /// This is what being roused means for a service, and it is the reason
    /// `reacts_to` exists. A stopped service is not restarted: it was not
    /// running before the file changed and starting it now would be a change
    /// nobody declared.
    ///
    /// Said by the command, because nothing can branch inside a batch. A plain
    /// `restart` starts a stopped service, and that is what was sent while this
    /// comment said otherwise. `try-restart` does nothing to a unit that is not
    /// running; `rc-service --ifstarted` was measured to exit 0 and do nothing
    /// for a stopped service.
    pub(crate) fn rouse(self, name: &str) -> Operation {
        let (program, arguments) = match self {
            Self::Systemd => ("systemctl", vec!["try-restart".to_owned(), name.to_owned()]),
            Self::OpenRc => (
                "rc-service",
                vec![
                    "--ifstarted".to_owned(),
                    name.to_owned(),
                    "restart".to_owned(),
                ],
            ),
        };
        Operation::Process {
            program: program.to_owned(),
            arguments,
            stdin: Vec::new(),
            private: false,
            expect: vec![0],
        }
    }

    /// The status question on its own, as something a batch can carry out.
    ///
    /// The first of [`Init::look`]'s observations is a program on both systems, so a
    /// batch can ask it again after acting. The second — `OpenRC`'s runlevel symlink —
    /// is a path, and the protocol has no operation that merely looks: the set is
    /// closed on purpose (`0033`). Rather than ask the same question two different
    /// ways in two places, which is two guesses that have to agree, the batch reads
    /// back only what it can, and drift in the other half is caught by the next
    /// plan's read like drift in anything else.
    pub(crate) fn watch(self, name: &str) -> Operation {
        let Some(Observation::Output { program, arguments }) = self.look(name).into_iter().next()
        else {
            unreachable!("a service is asked about by running a program on both systems")
        };
        Operation::Process {
            program,
            arguments,
            stdin: Vec::new(),
            // What a stopped service answers with is an answer, not a failure.
            expect: vec![0, 1, 3],
            private: false,
        }
    }

    /// Whether a service is running, from the answer to [`Init::watch`].
    pub(crate) fn running(self, name: &str, seen: &Seen) -> Result<Option<bool>, String> {
        self.understood(name, std::slice::from_ref(seen))
            .map(|found| found.map(|(running, _)| running))
    }
}

/// What the provider reports about a service that is there.
pub(crate) fn as_attributes(running: bool, enabled: bool) -> Vec<(String, Value)> {
    vec![
        ("running".to_owned(), Value::Boolean(running)),
        ("enabled".to_owned(), Value::Boolean(enabled)),
    ]
}
