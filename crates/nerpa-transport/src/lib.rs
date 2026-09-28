//! Carrying the wire to a machine, and knowing nothing about what it says.
//!
//! The executor at the far end is a filter: bytes in, bytes out, no socket of
//! its own (`0032`). So a transport is a *command that
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
use nerpa_proto::{Answer, Ask, At, Request};

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
    /// The thread draining what the far end writes to its standard error.
    ///
    /// Drained on a thread of its own, because a far end that fills that pipe
    /// and waits would otherwise hold the conversation. Kept because it is the
    /// only thing an executor of another protocol version says before it goes:
    /// it refuses the frame by complaining and exiting without an answer, and
    /// without this the report is a pipe that broke for no stated reason. See
    /// 0040 and RFC 0001 §5.3.
    ///
    /// Joined rather than peeked at. Reading whatever the thread had managed so
    /// far raced with it: the same test passed four times in five and the gate
    /// caught the fifth. The pipe ends when the far end does, so once the
    /// conversation has been ended the thread finishes on its own and what it
    /// returns is all there was.
    drain: Mutex<Option<std::thread::JoinHandle<Vec<u8>>>>,
    /// What that thread returned, once it has been joined.
    said: Mutex<Vec<u8>>,
    /// Whether the command never reached the far side at all.
    ///
    /// True when it ended with 255 — which is `ssh` failing to connect rather
    /// than anything on the machine speaking — and no byte of an answer had
    /// arrived. Both halves matter: a connection that was made and then broke
    /// has done something, and starting over would repeat it. See RFC 0001
    /// §5.3, and [`opened`], which is the only thing that asks.
    never_reached: Mutex<bool>,
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
            .stderr(Stdio::piped())
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

        let drain = child
            .stderr
            .take()
            .map(|complaining| std::thread::spawn(move || delivery::tail(complaining)));

        Ok(Self {
            node: node.clone(),
            described,
            talking: Mutex::new(Some(Talking {
                child,
                sending,
                reading,
            })),
            drain: Mutex::new(drain),
            said: Mutex::new(Vec::new()),
            never_reached: Mutex::new(false),
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

impl Piped {
    /// This machine being out of reach, with whatever the far end said about it.
    ///
    /// Everything that can go wrong in a conversation is the machine ceasing to
    /// be reachable: the pipe broke, the far end went away, a thread died
    /// holding it. That is the one failure a run survives, and it is about a
    /// machine rather than about any resource on it — so it is reported as what
    /// it is instead of being attributed to whatever was being asked about at
    /// the time.
    fn out_of_reach(&self, problem: &str) -> ProviderError {
        ProviderError::Unreachable {
            node: self.node.clone(),
            problem: format!("{}: {problem}{}", self.described, self.last_words()),
        }
    }

    /// Whether the command never reached the far side at all. See [`opened`].
    fn was_never_reached(&self) -> bool {
        self.never_reached.lock().is_ok_and(|never| *never)
    }

    /// What the far end said on its way out, if it said anything.
    ///
    /// Read when something has already gone wrong, so the draining thread has
    /// usually finished; what it has by then is what there is. An empty answer
    /// reads as nothing rather than as a gap, because a far end that went
    /// quietly is a different thing to report than one that explained itself.
    fn last_words(&self) -> String {
        // Joined here, and only here, because by now the far end has been ended
        // and its pipe closed: the thread returns what it read and stops. Asked
        // twice, the second answer comes from what the first one kept.
        if let Ok(mut drain) = self.drain.lock()
            && let Some(draining) = drain.take()
            && let Ok(words) = draining.join()
            && let Ok(mut kept) = self.said.lock()
        {
            *kept = words;
        }
        let Ok(kept) = self.said.lock() else {
            return String::new();
        };
        let words = String::from_utf8_lossy(&kept);
        let words = words.trim();
        if words.is_empty() {
            return String::new();
        }
        format!("; it said: {words}")
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
        let Ok(mut held) = self.talking.lock() else {
            return Err(self.out_of_reach("a thread died holding this connection"));
        };
        let Some(talking) = held.as_mut() else {
            return Err(self.out_of_reach("the far end has gone away"));
        };

        let exchanged = exchange(talking, ask);
        // Nothing arrived if the write never got out or the read got no bytes at
        // all; a frame that stopped part way is a far side that was speaking.
        let nothing_read = matches!(&exchanged, Err(nerpa_proto::WireError::Connection(_)));
        // A conversation that failed part way is out of step: what arrives next is
        // the rest of the frame that broke, or an answer to a question nobody is
        // waiting on. Kept, the next ask would read that as its own answer, or wait
        // for ever on a far end still busy with the last one. So it ends here, and
        // every later ask says so. Only `Drop` used to clear it; review found that.
        if exchanged.is_err()
            && let Some(broken) = held.take()
            && let Some(code) = end(broken)
            && code == 255
            && nothing_read
            && let Ok(mut never) = self.never_reached.lock()
        {
            *never = true;
        }
        // The lock goes before the words are asked for: ending the far end is
        // what lets the draining thread finish, and it is joined for them.
        drop(held);
        exchanged.map_err(|problem| self.out_of_reach(&problem.to_string()))
    }
}

/// One request and its answer, on a conversation that is still in step.
fn exchange(talking: &mut Talking, ask: &Ask) -> Result<Answer, nerpa_proto::WireError> {
    nerpa_proto::write(&mut talking.sending, &Request::new(ask.clone()))?;
    talking
        .sending
        .flush()
        .map_err(|error| nerpa_proto::WireError::Connection(error.to_string()))?;
    nerpa_proto::read_response(&mut talking.reading).map(|response| response.body().clone())
}

/// Ends a conversation that cannot go on. Its input is closed and the far end is
/// stopped rather than waited for, because it may be busy with something nobody
/// will read.
fn end(talking: Talking) -> Option<i32> {
    let Talking {
        mut child,
        sending,
        reading,
    } = talking;
    drop(sending);
    drop(reading);
    let _ = child.kill();
    // What it ended with, because 255 is how `ssh` says it never connected — the
    // one failure worth starting over for. A child this killed has no code of
    // its own, which reads as "it was reached and then stopped", and that is the
    // truth of it.
    child.wait().ok().and_then(|ended| ended.code())
}

/// Opens a machine and checks which build answered, asking again when `ssh`
/// could not connect at all.
///
/// The handshake is the next connection after delivery's, and it walks into the
/// same thing: a shared bastion drops connections at random once ten are being
/// made at once (RFC 0001 §5.10). Starting over is safe here and nowhere later —
/// nothing but `Build` has been written and no byte of an answer has been read,
/// so there is nothing to repeat and nothing half-done to tidy. Once the
/// conversation has carried anything else, a failure is a machine that went
/// away. See RFC 0001 §5.3.
///
/// # Errors
///
/// Returns [`ProviderError::Unreachable`] if the machine could not be opened, or
/// if what answered is not the build this nerpa carries.
pub fn opened(
    node: &nerpa_core::Target,
    program: &str,
    arguments: &[String],
    expected: Option<&str>,
) -> Result<Piped, ProviderError> {
    delivery::attempted(delivery::WAITS, || {
        let reach = Piped::running(node, program, arguments)
            // A command that will not start is this machine missing it, which
            // asking again will not fix.
            .map_err(|error| delivery::Refused::Answered(error.to_string()))?;
        let Some(build) = expected else {
            return Ok(reach);
        };
        match recognised(&reach, node, build) {
            Ok(()) => Ok(reach),
            Err(error) if reach.was_never_reached() => {
                Err(delivery::Refused::Connecting(error.to_string()))
            }
            Err(error) => Err(delivery::Refused::Answered(error.to_string())),
        }
    })
    .map_err(|refused| {
        let problem = match refused {
            delivery::Refused::Connecting(problem) | delivery::Refused::Answered(problem) => {
                problem
            }
        };
        ProviderError::Unreachable {
            node: node.clone(),
            problem,
        }
    })
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
            Ask::Perform {
                operations, seal, ..
            } => {
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
                            secret,
                        } => {
                            let entry = nerpa_proto::Entry::File {
                                content: content.clone(),
                                mode: *mode,
                            };
                            held.insert(path.clone(), entry.clone());
                            // Answered as a machine answers: what the path holds
                            // now, as a number, sealed when the write says it
                            // carries a secret.
                            let digest = nerpa_proto::digest(&Seen::Entry(entry));
                            became.push(match (*secret, seal) {
                                (false, _) => Seen::Digest { digest },
                                (true, Some(seal)) => Seen::Sealed {
                                    digest: nerpa_proto::sealed(seal, &digest),
                                },
                                (true, None) => {
                                    return Ok(Answer::Failed {
                                        at: At::Before,
                                        problem: "a write carrying a secret arrived without a \
                                                  seal, and its answer cannot be sealed"
                                            .to_owned(),
                                    });
                                }
                            });
                        }
                        Operation::Remove { path } => {
                            held.remove(path);
                            became.push(Seen::Nothing);
                        }
                        _ => {
                            return Ok(Answer::Failed {
                                at: At::Operation(became.len()),
                                problem: "a machine in memory cannot run a program".to_owned(),
                            });
                        }
                    }
                }
                Answer::Done { became }
            }
            _ => Answer::Failed {
                at: At::Before,
                problem: "a machine in memory does not answer that".to_owned(),
            },
        })
    }
}
