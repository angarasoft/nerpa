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

use std::io::Write as _;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Mutex;

use nerpa_core::provider::{ProviderError, Reach};
use nerpa_proto::{Answer, Ask, Request};

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
