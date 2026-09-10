//! Carrying the wire to a machine, and knowing nothing about what it says.
//!
//! The executor at the far end is a filter: bytes in, bytes out, no socket of
//! its own ([0032](../../../docs/decisions)). So a transport is a *command that
//! produces a pipe*, and the difference between running it here and running it
//! on a machine two continents away is which command.
//!
//! ```text
//!   here     Piped::running("target/release/nerpa-executor", [])
//!   there    Piped::running("ssh", ["web-01", "nerpa-executor"])
//!   with sudo  Piped::running("ssh", ["web-01", "sudo", "-n", "nerpa-executor"])
//! ```
//!
//! That third line is the whole of privilege escalation (`0035`). It is an
//! argument, not a protocol feature, and it is why there is no `become` on the
//! wire.
//!
//! # One process, many asks
//!
//! The child is started once and spoken to until it is dropped, so a run pays
//! for one `ssh` handshake rather than one per phase. Ansible's default is five
//! or six connections *per task*; pipelining brings that to one per task. This
//! is one per machine per run.

pub mod delivery;

/// How a build of the executor is named, for whoever holds the file it was
/// built as. See [`recognised`].
pub use nerpa_proto::build_of;

use std::io::Write as _;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Mutex;

use nerpa_core::provider::{ProviderError, Reach};
use nerpa_proto::{Answer, Ask, Request};

/// How long a machine has to answer before it counts as not answering.
///
/// `ssh` waits for the operating system's TCP timeout by default, which is two
/// minutes of a run doing nothing. A fleet with one machine switched off then
/// stalls rather than reporting it, which is the failure decision 0020 exists to
/// prevent — measured, not supposed: a plan against a machine that was not there
/// produced no output at all until this existed.
pub const ANSWERS_WITHIN: &str = "10";

/// How often a connection that has gone quiet is asked whether it is still
/// there, and how many times in a row it may not answer.
///
/// Not a timeout on the conversation. A batch may legitimately take minutes —
/// installing a package is not misbehaviour — so a deadline on reading would
/// kill the work it was meant to protect. What can be bounded is whether the
/// path is still alive, and `ssh` is the thing that knows: thirty seconds, three
/// times, so a machine whose network went away is reported inside two minutes
/// instead of holding the run until the operating system gives up.
pub const ALIVE_EVERY: &str = "30";

/// How many unanswered checks end it. See [`ALIVE_EVERY`].
pub const ALIVE_MISSES: &str = "3";

/// A machine reached by running a command that speaks the wire on its pipes.
#[derive(Debug)]
pub struct Piped {
    /// Which machine this is, so a failure can be reported as that machine
    /// being out of reach rather than as something about a resource.
    node: nerpa_core::Target,
    /// Named so a failure can say which command did not work, without keeping
    /// the child alive to ask it.
    described: String,
    /// `None` once the far end has gone away, so a second ask says that rather
    /// than blocking on a pipe nobody is reading.
    talking: Mutex<Option<Talking>>,
}

#[derive(Debug)]
struct Talking {
    child: Child,
    sending: ChildStdin,
    reading: ChildStdout,
}

impl Piped {
    /// Reaches whatever this command's standard input and output are attached to.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::Unreachable`] if the command could not be
    /// started at all: on a machine that is what "could not be reached" means,
    /// and it is the one failure a run survives.
    pub fn running(
        node: &nerpa_core::Target,
        program: &str,
        arguments: &[String],
    ) -> Result<Self, ProviderError> {
        let described = format!("{program} {}", arguments.join(" "));
        let mut child = Command::new(program)
            .args(arguments)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|error| ProviderError::Unreachable {
                node: node.clone(),
                problem: format!("{described}: {error}"),
            })?;

        // Taken together or not at all: a child with one pipe missing is one
        // this cannot hold a conversation with, and pretending otherwise would
        // deadlock rather than fail.
        let (Some(sending), Some(reading)) = (child.stdin.take(), child.stdout.take()) else {
            let _ = child.kill();
            return Err(ProviderError::Unreachable {
                node: node.clone(),
                problem: format!("{described}: its pipes could not be opened"),
            });
        };

        Ok(Self {
            node: node.clone(),
            described,
            talking: Mutex::new(Some(Talking {
                child,
                sending,
                reading,
            })),
        })
    }
}

impl Drop for Piped {
    fn drop(&mut self) {
        // Closing the input is how the far end is told the run is over; it ends
        // by itself. Waited for so that a finished run leaves no child behind.
        if let Ok(mut held) = self.talking.lock()
            && let Some(mut talking) = held.take()
        {
            drop(talking.sending);
            let _ = talking.child.wait();
        }
    }
}

impl Reach for Piped {
    fn ask(&self, ask: &Ask) -> Result<Answer, ProviderError> {
        // Everything that can go wrong here is the machine ceasing to be
        // reachable: the pipe broke, the far end went away, a thread died
        // holding it. That is the one failure a run survives, and it is about a
        // machine rather than about any resource on it — so it is reported as
        // what it is instead of being attributed to whatever was being asked
        // about at the time.
        let out_of_reach = |problem: String| ProviderError::Unreachable {
            node: self.node.clone(),
            problem: format!("{}: {problem}", self.described),
        };

        let mut held = self
            .talking
            .lock()
            .map_err(|_| out_of_reach("a thread died holding this connection".to_owned()))?;
        let Some(talking) = held.as_mut() else {
            return Err(out_of_reach("the far end has gone away".to_owned()));
        };

        nerpa_proto::write(&mut talking.sending, &Request::new(ask.clone()))
            .map_err(|error| out_of_reach(error.to_string()))?;
        talking
            .sending
            .flush()
            .map_err(|error| out_of_reach(error.to_string()))?;

        nerpa_proto::read_response(&mut talking.reading)
            .map(|response| response.body().clone())
            .map_err(|error| out_of_reach(error.to_string()))
    }
}

// ENFORCES: INV-WIRE-003
/// Whether the far end is the executor this nerpa carries.
///
/// Asked before anything else is, because the executor is what acts on a
/// machine — often as root — and a fix to it changes nothing the protocol
/// version can see. An executor installed before the fix speaks the same wire and
/// answers every question; only asking which build it is tells the two apart.
/// See decision 0040.
///
/// Refused as out of reach, the way a machine of the wrong architecture is
/// refused at delivery: the machine answered, and nothing this nerpa ships is
/// there to do the work, so its resources are reported unread and every other
/// machine carries on. The refusal names both builds, because the way out is
/// installing one of them.
///
/// # Errors
///
/// [`ProviderError::Unreachable`] if the far end names another build, cannot say
/// which it is, or could not be asked at all.
pub fn recognised(
    reach: &dyn Reach,
    node: &nerpa_core::Target,
    carried: &str,
) -> Result<(), ProviderError> {
    let refused = |problem: String| ProviderError::Unreachable {
        node: node.clone(),
        problem,
    };
    match reach.ask(&Ask::Build)? {
        Answer::Build { digest } if digest == carried => Ok(()),
        Answer::Build { digest } => Err(refused(format!(
            "the executor there is build {}, and this nerpa carries {}; \
             install the one it carries",
            short(&digest),
            short(carried)
        ))),
        Answer::Failed { problem, .. } => Err(refused(format!(
            "the executor there did not say which build it is: {problem}"
        ))),
        other => Err(refused(format!(
            "the executor there was asked which build it is and answered {other:?}"
        ))),
    }
}

/// Enough of a digest to tell two builds apart in a sentence.
fn short(digest: &str) -> &str {
    digest.get(..16).unwrap_or(digest)
}

/// The executor, in this process.
///
/// The same code the far end runs, without a pipe between. For `localhost`,
/// where a subprocess buys nothing — nerpa is already on the machine it would be
/// talking to — and for tests of everything above the wire.
///
/// It does not exercise the wire, deliberately. That is covered where the wire
/// is what is under test: a test that goes through a pipe to prove a provider
/// works is really testing the pipe, badly.
#[derive(Debug, Default, Clone, Copy)]
pub struct Here;

impl Reach for Here {
    fn ask(&self, ask: &Ask) -> Result<Answer, ProviderError> {
        Ok(nerpa_executor::answer(ask))
    }
}

/// A machine that exists only in memory.
///
/// The fake the provider used to carry itself. It moved here because that is
/// where the world is now: a provider that only translates has nothing in it to
/// stand in for, and a test that wants no disk touched wants a far end that has
/// no disk.
///
/// It answers the same protocol and keeps what it is told, which is enough for
/// anything above the wire. It is not the executor and does not try to be —
/// there is no mode, no symlink, no process. A test that needs those needs
/// [`Here`], which is the executor.
#[derive(Debug, Default)]
pub struct Held {
    entries: Mutex<std::collections::BTreeMap<String, nerpa_proto::Entry>>,
}

impl Held {
    /// A machine with nothing on it.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// What it holds now, so a test can look without going through the wire.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.lock().map_or(0, |held| held.len())
    }

    /// Whether it holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Reach for Held {
    fn ask(&self, ask: &Ask) -> Result<Answer, ProviderError> {
        use nerpa_proto::{Observation, Operation, Seen};

        let Ok(mut held) = self.entries.lock() else {
            return Err(ProviderError::NotOnAMachine);
        };

        Ok(match ask {
            Ask::Look { observations, .. } => Answer::Found {
                seen: observations
                    .iter()
                    .map(|observation| match observation {
                        Observation::Path { path } => {
                            held.get(path).cloned().map_or(Seen::Nothing, Seen::Entry)
                        }
                        // A machine in memory runs nothing. Saying so is better
                        // than answering as though it had.
                        _ => Seen::Nothing,
                    })
                    .collect(),
            },
            Ask::Perform { operations, .. } => {
                let mut became = Vec::with_capacity(operations.len());
                for operation in operations {
                    match operation {
                        Operation::Directory { path, mode } => {
                            let entry = nerpa_proto::Entry::Directory { mode: *mode };
                            held.insert(path.clone(), entry.clone());
                            became.push(Seen::Entry(entry));
                        }
                        Operation::File {
                            path,
                            content,
                            mode,
                        } => {
                            let entry = nerpa_proto::Entry::File {
                                content: content.clone(),
                                mode: *mode,
                            };
                            held.insert(path.clone(), entry.clone());
                            became.push(Seen::Entry(entry));
                        }
                        Operation::Remove { path } => {
                            held.remove(path);
                            became.push(Seen::Nothing);
                        }
                        _ => {
                            return Ok(Answer::Failed {
                                at: became.len(),
                                problem: "a machine in memory cannot run a program".to_owned(),
                            });
                        }
                    }
                }
                Answer::Done { became }
            }
            _ => Answer::Failed {
                at: 0,
                problem: "a machine in memory does not answer that".to_owned(),
            },
        })
    }
}
