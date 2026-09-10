//! The wire between the engine and an executor on a machine.
//!
//! Expands: 0010
//!
//! Everything the engine wants done on a node crosses this, and nothing else
//! does. The transport underneath carries bytes and knows nothing about their
//! meaning ([0010](../../../docs/decisions)); this crate is that meaning, and
//! the executor at the far end is the only thing that acts on it.
//!
//! # How the pieces fit
//!
//! ```text
//!   engine ── Request ──┐                    ┌── perform ── the machine
//!                       │  frame: length     │
//!                       ├─ then CBOR ────────┤
//!                       │                    │
//!          ── Response ─┘                    └── executor
//! ```
//!
//! # Why this is settled carefully
//!
//! A wire protocol is a compatibility surface: once an executor built from one
//! version is installed somewhere, every later engine has to keep talking to it.
//! So every frame carries the version it was written by, and a version that is
//! not understood is refused rather than guessed at.
//!
//! # A length prefix is an instruction to allocate
//!
//! It comes from the other end of a pipe, and believing it is how a peer that is
//! confused — or hostile — turns a length into memory exhaustion. Frames are
//! bounded, and a prefix over the bound is refused before a byte is reserved.

use std::io::{Read, Write};

use serde::{Deserialize, Serialize};

/// The version every frame is stamped with.
///
/// Bumped when the meaning of anything below changes. An executor that reads a
/// version it does not know refuses the frame; it does not attempt a subset.
///
/// Three since an executor can be asked which build it is. The bump is what
/// makes an executor from before that refuse the first frame by name, rather
/// than answer everything else as though nothing had changed.
pub const PROTOCOL_VERSION: u32 = 3;

/// The largest frame either end will read.
///
/// Generous, because a file's contents travel inside one, and bounded, because
/// the length that says how big it is arrives from somewhere else.
pub const MAX_FRAME: usize = 64 * 1024 * 1024;

/// What went wrong on the wire.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum WireError {
    /// The pipe ended, or would not carry.
    #[error("the connection failed: {0}")]
    Connection(String),
    /// The pipe ended in the middle of a frame.
    #[error("the connection ended after {read} bytes of a {expected}-byte frame")]
    Truncated {
        /// How much arrived.
        read: usize,
        /// How much the prefix promised.
        expected: usize,
    },
    /// The frame claimed a size this end will not allocate.
    #[error("a frame of {size} bytes was announced, and {MAX_FRAME} is the most that is read")]
    TooLarge {
        /// What the prefix claimed.
        size: usize,
    },
    /// The bytes were not what they said they were.
    #[error("the frame could not be read: {0}")]
    Malformed(String),
    /// Written by something this end does not understand.
    #[error(
        "the frame was written by protocol version {found}, and this is version {PROTOCOL_VERSION}"
    )]
    Version {
        /// What the other end stamped it with.
        found: u32,
    },
}

/// What the engine asks for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    version: u32,
    body: Ask,
}

impl Request {
    /// Stamps an ask with this build's protocol version.
    #[must_use]
    pub fn new(body: Ask) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            body,
        }
    }

    /// What is being asked.
    #[must_use]
    pub fn body(&self) -> &Ask {
        &self.body
    }

    /// The version it was written by.
    #[must_use]
    pub fn version(&self) -> u32 {
        self.version
    }
}

/// The things an executor can be asked to do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Ask {
    /// What do these look like now?
    ///
    /// A batch, because the whole reason this wire exists is that one journey to
    /// a machine should answer for everything on it.
    Look {
        /// Everything on this machine worth looking at, in one go. Answered by
        /// [`Answer::Found`], one [`Seen`] for each, in this order.
        observations: Vec<Observation>,
    },
    /// Make these so, in the order given.
    Perform {
        /// Which plan this came from, or `None` if that plan had none.
        ///
        /// A plan that could not look at everything has no number, and one of
        /// those can still be applied to the machines that did answer — so this
        /// is optional on the wire and refusing it is a policy above the wire
        /// rather than a shape below it. See `INV-PLAN-006`.
        plan: Option<[u8; 32]>,
        /// What the plan was computed from, and what it hashed to.
        ///
        /// Checked before anything is done. Between the looking and this, the
        /// machine can change; without this the batch would act on a reading
        /// that is minutes old and possibly wrong. See decision 0034.
        premise: Vec<Premise>,
        /// Carried out in the order given, and stopping at the first failure.
        operations: Vec<Operation>,
    },
    /// What did you last apply, as one number?
    ///
    /// The cheapest of the three tiers in decision 0011: a node whose digest
    /// matches what state remembers is skipped whole.
    Fingerprint,
    /// Which build are you?
    ///
    /// Asked before anything else by an engine that has to know it is talking
    /// to the executor it ships. The protocol version cannot say that: a fix
    /// inside the executor changes nothing on the wire, so an executor without
    /// it answers every frame as though it had it. See decision 0040.
    Build,
}

/// Something the engine wants looked at.
///
/// Reading and acting are the same shape on purpose: a small closed set that
/// includes running a program. A provider that has to run `qlist -I` to know
/// what is installed needs it as much as one that has to run `emerge` to change
/// it, and a wire that allowed only one of the two would send every package
/// provider looking for a back door. See decision 0033.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Observation {
    /// What is at this path.
    Path {
        /// Where to look.
        path: String,
    },
    /// What this path holds, following a link to get there.
    ///
    /// A different question to [`Observation::Path`], which asks *what is at*
    /// this path. Reporting a link rather than following it is right when the
    /// path is being managed — a record of one object's mode with another's
    /// contents describes nothing that exists. It is wrong when the path is
    /// being read: `/etc/os-release` is a link on most machines, and what the
    /// asker wants is what it says.
    Contents {
        /// Where to read.
        path: String,
    },
    /// What this program says when it is run.
    Output {
        /// The program, found on the far end's `PATH` or named absolutely.
        program: String,
        /// Its arguments, one to an entry and never a single shell string — a
        /// shell string is where somebody else's quoting becomes somebody
        /// else's command.
        arguments: Vec<String>,
    },
}

/// What one observation found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Seen {
    /// There is nothing at that path.
    ///
    /// Only ever said when the far end is sure. A path it was not allowed to
    /// read is an [`Answer::Failed`], because a file nobody may open is not a
    /// file that is not there — the distinction `INV-READ-001` makes on the
    /// other side of this wire.
    Nothing,
    /// This is what is at the path.
    Entry(Entry),
    /// The program ran, and this is what it said.
    Said {
        /// What it exited with.
        status: i32,
        /// What it printed, as bytes: not everything a program prints is text.
        #[serde(with = "serde_bytes")]
        out: Vec<u8>,
        /// What it printed on the other stream.
        #[serde(with = "serde_bytes")]
        complaint: Vec<u8>,
    },
}

/// Something that was true when the plan was made and has to still be true.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Premise {
    /// What to look at again.
    pub observation: Observation,
    /// What it hashed to when the plan was computed, by [`digest`].
    pub digest: [u8; 32],
}

/// One thing to make true on a machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Operation {
    /// A directory, with these permissions.
    Directory {
        /// Where.
        path: String,
        /// Permission bits.
        mode: u32,
    },
    /// A file, with these contents and permissions.
    File {
        /// Where.
        path: String,
        /// What is in it.
        ///
        /// Bytes, not text. A certificate, a keytab, a file in an encoding that
        /// is not UTF-8 — each is something somebody manages, and a `String`
        /// cannot carry any of them. CBOR was chosen over JSON for exactly this
        /// and had not been asked to do it; `serde_bytes` is what makes it a
        /// byte string on the wire rather than an array of numbers.
        #[serde(with = "serde_bytes")]
        content: Vec<u8>,
        /// Permission bits.
        mode: u32,
    },
    /// Run a program, and take what it does as the change.
    ///
    /// Installing a package *is* running `emerge`; there is no honest way to
    /// describe it that does not end there. So the set stays small and general
    /// rather than growing a variant per kind of resource, which is what lets it
    /// be frozen at all.
    ///
    /// Emitted by providers and never by the author of a configuration
    /// (decision 0033). The moment a person can write one of these, this is
    /// Terraform's `remote-exec` and deserves everything said about it.
    Process {
        /// The program, found on the far end's `PATH` or named absolutely.
        program: String,
        /// Its arguments, one to an entry and never a single shell string.
        arguments: Vec<String>,
        /// What to write to it, if anything.
        ///
        /// The only place a secret may go. `/proc/*/cmdline` is world-readable,
        /// so a credential in `arguments` is a credential anybody on the machine
        /// can read while it runs.
        #[serde(with = "serde_bytes")]
        stdin: Vec<u8>,
        /// Whether what is written to it must never be recorded.
        ///
        /// The far end keeps it out of anything it logs, and so does this end.
        /// See decision 0027.
        private: bool,
        /// Which exit codes mean it worked. Empty means only zero does.
        expect: Vec<i32>,
    },
    /// Gone.
    Remove {
        /// What to take away.
        path: String,
    },
}

impl Operation {
    /// Where this operation acts, when that is a place.
    ///
    /// A process acts wherever it acts, and this wire has no way to say where —
    /// which is the honest answer rather than a missing feature. Anything
    /// reporting *which* operation stopped a batch uses its index; this is for
    /// the things that genuinely have a path.
    #[must_use]
    pub fn path(&self) -> Option<&str> {
        match self {
            Operation::Directory { path, .. }
            | Operation::File { path, .. }
            | Operation::Remove { path } => Some(path),
            Operation::Process { .. } => None,
        }
    }
}

/// What the executor says back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Response {
    version: u32,
    body: Answer,
}

impl Response {
    /// Stamps an answer with this build's protocol version.
    #[must_use]
    pub fn new(body: Answer) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            body,
        }
    }

    /// What was answered.
    #[must_use]
    pub fn body(&self) -> &Answer {
        &self.body
    }

    /// The version it was written by.
    #[must_use]
    pub fn version(&self) -> u32 {
        self.version
    }
}

/// What an executor can answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Answer {
    /// What each observation found, in the order they were asked about.
    Found {
        /// One for each [`Observation`], and always the same number of them.
        seen: Vec<Seen>,
    },
    /// Everything asked for was done, and this is what each became.
    Done {
        /// One for each [`Operation`], in the order they were given.
        ///
        /// Read back rather than echoed, so that writing nothing is noticed —
        /// and the same shape as [`Answer::Found`], because a process has no
        /// path to key an answer by any more than an observation of one does.
        became: Vec<Seen>,
    },
    /// The machine is not what the plan was computed from, so nothing was done.
    ///
    /// Not a failure. The plan is reasoning from a reading that has since gone
    /// stale, and the answer is to look again and plan again — which is a
    /// different thing to tell somebody than that an operation went wrong.
    Moved {
        /// Which premise no longer holds, as an index into the batch's.
        at: usize,
    },
    /// One number standing for everything the executor last applied.
    Fingerprint {
        /// Hexadecimal digest.
        digest: String,
    },
    /// Which build answered, named by [`build_of`] over its own binary.
    Build {
        /// Hexadecimal digest.
        digest: String,
    },
    /// It could not be done, and this is as far as it got.
    Failed {
        /// Which operation or observation stopped it, by index.
        ///
        /// An index rather than a path, because not everything either end talks
        /// about has one — and because it doubles as how many went through
        /// before it, so the engine knows what happened rather than assuming
        /// nothing did.
        at: usize,
        /// What the machine said.
        problem: String,
    },
}

/// What is at a path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Entry {
    /// A directory.
    Directory {
        /// Permission bits.
        mode: u32,
    },
    /// A file.
    File {
        /// What is in it, as bytes. See [`Operation::File`].
        #[serde(with = "serde_bytes")]
        content: Vec<u8>,
        /// Permission bits.
        mode: u32,
    },
    /// A pointer somewhere else.
    ///
    /// Reported, never followed. Reading through a link describes one object and
    /// writing through it changes another, and a record of the two together is a
    /// record of something that does not exist.
    Symlink {
        /// What it points at, as the machine has it.
        target: String,
    },
}

/// One number standing for what an observation found.
///
/// Computed identically on both ends, which is what lets a [`Premise`] be
/// checked where the machine is rather than where the plan was made. Written out
/// rather than borrowed from the serialisation format because it has to mean the
/// same thing for ever, and every variable-length part carries its length so
/// that no two different answers can encode alike.
#[must_use]
pub fn digest(seen: &Seen) -> [u8; 32] {
    let mut out = Vec::from(b"nerpa-seen-1".as_slice());
    match seen {
        Seen::Nothing => out.push(0),
        Seen::Entry(entry) => {
            out.push(1);
            match entry {
                Entry::Directory { mode } => {
                    out.push(1);
                    out.extend_from_slice(&mode.to_le_bytes());
                }
                Entry::File { content, mode } => {
                    out.push(2);
                    out.extend_from_slice(&length(content.len()));
                    out.extend_from_slice(content);
                    out.extend_from_slice(&mode.to_le_bytes());
                }
                Entry::Symlink { target } => {
                    out.push(3);
                    out.extend_from_slice(&length(target.len()));
                    out.extend_from_slice(target.as_bytes());
                }
            }
        }
        Seen::Said {
            status,
            out: said,
            complaint,
        } => {
            out.push(2);
            out.extend_from_slice(&status.to_le_bytes());
            out.extend_from_slice(&length(said.len()));
            out.extend_from_slice(said);
            out.extend_from_slice(&length(complaint.len()));
            out.extend_from_slice(complaint);
        }
    }
    *blake3::hash(&out).as_bytes()
}

/// The name a build of the executor goes by: a digest of every byte of it.
///
/// Computed the same way by the executor over itself and by the engine over the
/// file it carries, which is what lets the two be compared rather than trusted.
/// Derived rather than declared, because a number somebody has to remember to
/// change is a number that stays the same across a fix.
#[must_use]
pub fn build_of(binary: &[u8]) -> String {
    blake3::hash(binary).to_hex().to_string()
}

/// A length as eight canonical bytes.
///
/// `usize` is never wider than `u64` on any target this builds for, so the
/// fallback is unreachable; it is written rather than unwrapped because a digest
/// is not a place to put a panic.
fn length(count: usize) -> [u8; 8] {
    u64::try_from(count).unwrap_or(u64::MAX).to_le_bytes()
}

/// Whether a frame of this size may be handled at all.
///
/// Extracted so the boundary can be tested at the boundary. Asserting it by
/// building a frame of exactly [`MAX_FRAME`] would allocate sixty-four megabytes
/// twice over, which is a steep price for one comparison.
fn permitted(size: usize) -> Result<usize, WireError> {
    if size > MAX_FRAME {
        return Err(WireError::TooLarge { size });
    }
    Ok(size)
}

/// Writes one frame: four bytes of length, then the value as CBOR.
///
/// # Errors
///
/// Returns [`WireError`] if the value cannot be encoded or the pipe will not
/// take it.
pub fn write<W: Write, T: Serialize>(sink: &mut W, value: &T) -> Result<(), WireError> {
    let mut body = Vec::new();
    ciborium::into_writer(value, &mut body)
        .map_err(|error| WireError::Malformed(error.to_string()))?;

    let size = u32::try_from(permitted(body.len())?)
        .map_err(|_| WireError::TooLarge { size: body.len() })?;
    sink.write_all(&size.to_be_bytes())
        .and_then(|()| sink.write_all(&body))
        .and_then(|()| sink.flush())
        .map_err(|error| WireError::Connection(error.to_string()))
}

/// Reads one frame.
///
/// # Errors
///
/// Returns [`WireError`] if the pipe ends early, announces more than
/// [`MAX_FRAME`], or carries something that is not the expected shape.
pub fn read<R: Read, T: for<'de> Deserialize<'de>>(source: &mut R) -> Result<T, WireError> {
    let mut prefix = [0_u8; 4];
    source
        .read_exact(&mut prefix)
        .map_err(|error| WireError::Connection(error.to_string()))?;

    // ENFORCES: INV-WIRE-002
    // The size arrives from the other end of a pipe. Checked before a byte is
    // reserved, because believing it is how a confused peer turns a number into
    // memory exhaustion.
    let announced = u32::from_be_bytes(prefix);
    let announced = usize::try_from(announced).map_err(|_| WireError::TooLarge {
        size: MAX_FRAME.saturating_add(1),
    })?;
    let size = permitted(announced)?;

    // Filled by reading into whatever is left of it, so a short read leaves a
    // partial frame rather than a lie about how much arrived.
    let mut body = vec![0_u8; size];
    let mut read = 0;
    while read < size {
        let Some(rest) = body.get_mut(read..) else {
            return Err(WireError::Truncated {
                read,
                expected: size,
            });
        };
        match source.read(rest) {
            Ok(0) => {
                return Err(WireError::Truncated {
                    read,
                    expected: size,
                });
            }
            Ok(more) => read += more,
            Err(error) => return Err(WireError::Connection(error.to_string())),
        }
    }

    ciborium::from_reader(body.as_slice()).map_err(|error| WireError::Malformed(error.to_string()))
}

/// Reads a request, and refuses one written by a version this build does not know.
///
/// # Errors
///
/// Everything [`read`] returns, plus [`WireError::Version`].
// ENFORCES: INV-WIRE-001
pub fn read_request<R: Read>(source: &mut R) -> Result<Request, WireError> {
    let request: Request = read(source)?;
    if request.version != PROTOCOL_VERSION {
        return Err(WireError::Version {
            found: request.version,
        });
    }
    Ok(request)
}

/// Reads a response, and refuses one written by a version this build does not know.
///
/// # Errors
///
/// Everything [`read`] returns, plus [`WireError::Version`].
pub fn read_response<R: Read>(source: &mut R) -> Result<Response, WireError> {
    let response: Response = read(source)?;
    if response.version != PROTOCOL_VERSION {
        return Err(WireError::Version {
            found: response.version,
        });
    }
    Ok(response)
}

// The boundary, asserted at the boundary rather than by building a frame of that
// size. Mutation testing found it: `>` and `>=` are indistinguishable everywhere
// except at exactly the limit.
#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "tests may be blunt: see docs/CODING_STANDARDS.md"
)]
mod tests {
    use super::{MAX_FRAME, WireError, permitted};

    #[test]
    fn a_frame_of_exactly_the_limit_is_allowed() {
        assert_eq!(permitted(MAX_FRAME).ok(), Some(MAX_FRAME));
        assert_eq!(permitted(MAX_FRAME - 1).ok(), Some(MAX_FRAME - 1));
        assert_eq!(permitted(0).ok(), Some(0));
    }

    #[test]
    fn one_byte_past_the_limit_is_not() {
        let refusal = permitted(MAX_FRAME + 1).unwrap_err();
        assert!(
            matches!(refusal, WireError::TooLarge { size } if size == MAX_FRAME + 1),
            "{refusal:?}"
        );
    }
}
