//! The wire between the engine and an executor on a machine.
//!
//! Expands: 0010
//!
//! Everything the engine wants done on a node crosses this, and nothing else
//! does. The transport underneath carries bytes and knows nothing about their
//! meaning (`0010`); this crate is that meaning, and
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
/// Three since an executor can be asked which build it is. An executor of
/// version 2 refuses the first frame of version 3, and says it is a request it
/// has never heard of rather than a version it does not know: it decoded the
/// whole frame before looking at the stamp. Measured, on a machine still holding
/// one. From three on, the stamp is read first.
///
/// Four since a premise travels sealed for the machine it describes, and an
/// observation can be asked for as a seal rather than as the thing. See decision
/// 0041.
#[dacc_derive::doc_anchor(id = "wire-protocol")]
pub const PROTOCOL_VERSION: u32 = 5;

/// The largest frame either end will read.
///
/// Generous, because a file's contents travel inside one, and bounded, because
/// the length that says how big it is arrives from somewhere else.
pub const MAX_FRAME: usize = 64 * 1024 * 1024;

/// The largest content one operation may carry.
///
/// Half a frame, because what is written is read back: a file's answer holds its
/// content again, and both have to fit in a frame with room for everything else.
/// Checked when planning, so a file too big to send is refused before anything
/// happens rather than when its batch is written.
pub const MAX_CONTENT: usize = MAX_FRAME / 2;

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
#[serde(deny_unknown_fields)]
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
#[serde(rename_all = "snake_case", deny_unknown_fields)]
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
        /// What an [`Observation::Sealed`] among them is sealed with. `None` when
        /// nothing asked for is sealed; a sealed one without it is refused.
        seal: Option<Seal>,
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
        /// What the premise is sealed with, for this machine.
        ///
        /// Put on by the engine at the moment it asks and never carried in the
        /// plan, so a plan's JSON holds digests nobody without the state can test
        /// a guess against. A premise that arrives without it is refused rather
        /// than compared bare. See decision 0041.
        seal: Option<Seal>,
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
#[serde(rename_all = "snake_case", deny_unknown_fields)]
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
    /// What another observation found, sealed for this machine, and not the
    /// thing itself.
    ///
    /// For a value the engine has to compare and must not hold: a password hash
    /// in `/etc/shadow`, a secret file's contents. The far end reads it, seals
    /// its digest with the seal the request carries, and answers with only that.
    /// One inside another is refused. See decision 0041.
    Sealed {
        /// What to look at.
        of: Box<Observation>,
    },
}

/// What one observation found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
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
    /// What an [`Observation::Sealed`] found, as its seal. See [`sealed`].
    Sealed {
        /// The sealed digest.
        digest: [u8; 32],
    },
    /// What a private process did, without what it said.
    ///
    /// Its status, because that is what decides whether the operation worked,
    /// and a digest of everything it printed — sealed for the machine, so the
    /// engine can compare it against a run it expected and nobody else can test
    /// guesses against it. A program handed a credential on its input usually
    /// prints something about it, and this is where that stopped travelling
    /// home. See decision 0027.
    Private {
        /// What it exited with.
        status: i32,
        /// What it printed, hashed and sealed.
        sealed: [u8; 32],
    },
    /// What a write left behind, as what it hashed to.
    ///
    /// A file write is answered with this rather than with what was read back.
    /// The far end still looks at the path after writing it, so writing nothing
    /// is still noticed; what changes is what comes home. The engine holds what
    /// it sent and compares, so the bytes have no reason to travel back — and
    /// for a large file they would be the file a second time.
    Digest {
        /// What is at the path now, hashed.
        digest: [u8; 32],
    },
}

/// Something that was true when the plan was made and has to still be true.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Premise {
    /// What to look at again.
    pub observation: Observation,
    /// What it hashed to when the plan was computed, by [`digest`], sealed for
    /// the machine by [`sealed`].
    pub digest: [u8; 32],
}

/// One thing to make true on a machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
#[non_exhaustive]
#[dacc_derive::doc_anchor(id = "operation")]
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
        /// Whether what is written here is a secret.
        ///
        /// Required, with no default: the far end answers a secret write with a
        /// sealed digest rather than with what it read back, and a write that
        /// forgot to say so would have the machine answer with the value. A
        /// `Perform` carrying one without a seal is refused before anything
        /// runs. See decision 0027 and `INV-PREMISE-002`.
        secret: bool,
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

/// Where a batch stopped.
///
/// Every one of these was the index zero once, which made a refusal before
/// anything was looked at indistinguishable from a failure at the first premise:
/// the engine could not tell a batch that never started from one that got as far
/// as its first check. An index alone cannot say that, because the places are not
/// one sequence.
///
/// The number is an index rather than a path, because not everything either end
/// talks about has one — and it doubles as how many went through before it, so
/// the engine knows what happened rather than assuming nothing did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
#[non_exhaustive]
pub enum At {
    /// Nothing was looked at and nothing was done. A request refused for what it
    /// was, such as a premise or a secret write that arrived without its seal.
    Before,
    /// Checking the premise with this index, in a `Perform`.
    Premise(usize),
    /// Carrying out the operation with this index, in a `Perform`.
    Operation(usize),
    /// Looking at the observation with this index, in a `Look`.
    Observation(usize),
}

impl std::fmt::Display for At {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            At::Before => write!(f, "before anything was done"),
            At::Premise(at) => write!(f, "at premise {at}"),
            At::Operation(at) => write!(f, "at operation {at}"),
            At::Observation(at) => write!(f, "at observation {at}"),
        }
    }
}

/// What the executor says back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
#[serde(rename_all = "snake_case", deny_unknown_fields)]
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
        /// Where it stopped.
        at: At,
        /// What the machine said.
        problem: String,
    },
}

/// What is at a path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
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
        Seen::Sealed { digest: sealed } => {
            out.push(3);
            out.extend_from_slice(sealed);
        }
        Seen::Digest { digest: written } => {
            out.push(4);
            out.extend_from_slice(written);
        }
        Seen::Private { status, sealed } => {
            out.push(5);
            out.extend_from_slice(&status.to_le_bytes());
            out.extend_from_slice(sealed);
        }
    }
    *blake3::hash(&out).as_bytes()
}

/// What a premise and a sealed observation are sealed with, for one machine.
///
/// Derived by the engine from its state file's key and the machine's name, so a
/// machine holding its own seal can check what it was asked about and nothing
/// else: not another machine's premise, not a secret's digest in state.
/// Deliberately opaque when printed, because a seal in a log is a key in a log.
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Seal(pub [u8; 32]);

impl std::fmt::Debug for Seal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Seal(…)")
    }
}

/// A digest sealed for one machine: keyed, so that holding it without the seal
/// tells nobody what it was taken over.
///
/// Computed identically on both ends. The engine seals what the plan read; the
/// executor seals what it reads again, and compares. See decision 0041.
#[must_use]
pub fn sealed(seal: &Seal, digest: &[u8; 32]) -> [u8; 32] {
    *blake3::keyed_hash(&seal.0, digest).as_bytes()
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
    decode(&frame(source)?)
}

/// The bytes of one frame, before anything in them is believed.
#[dacc_derive::doc_anchor(id = "inv-wire-002")]
fn frame<R: Read>(source: &mut R) -> Result<Vec<u8>, WireError> {
    let mut prefix = [0_u8; 4];
    source
        .read_exact(&mut prefix)
        .map_err(|error| WireError::Connection(error.to_string()))?;

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
            // A signal arrived before anything did, which is not the pipe ending.
            // The prefix is read with `read_exact`, which asks again; the body
            // called this a lost connection, so a run handling signals would have
            // lost a machine in the middle of its answer to its own Ctrl-C.
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) => return Err(WireError::Connection(error.to_string())),
        }
    }

    Ok(body)
}

/// What a frame's bytes say, as the shape asked for, to the last byte of it.
///
/// Read to the end rather than until the shape is satisfied. Trailing bytes
/// inside a frame were ignored, and bytes nobody looks at are bytes a peer has
/// somewhere to put: the length prefix says how much this end agreed to read, so
/// everything inside it is either the shape or a disagreement about what the
/// shape is.
fn decode<T: for<'de> Deserialize<'de>>(body: &[u8]) -> Result<T, WireError> {
    let mut source = std::io::Cursor::new(body);
    let value = ciborium::from_reader(&mut source)
        .map_err(|error| WireError::Malformed(error.to_string()))?;
    let read = usize::try_from(source.position()).unwrap_or(usize::MAX);
    if read != body.len() {
        return Err(WireError::Malformed(format!(
            "the shape ended after {read} bytes and the frame announced {}",
            body.len()
        )));
    }
    Ok(value)
}

/// A frame's version, and nothing else about it.
///
/// Read on its own before the rest is decoded. A frame from another version can
/// carry a request this build has never heard of, and decoding all of it first
/// reports that as an unknown variant — which is what an executor of version 2
/// said to the first frame of version 3. The version is the reason; the variant
/// is only how it showed.
/// Deliberately not `deny_unknown_fields`: this reads one field out of a frame
/// that carries more, and the rest of it is the body this has not decided to
/// believe yet.
#[derive(Deserialize)]
struct Stamp {
    version: u32,
}

/// Refuses a frame stamped with a version this build does not know.
fn stamped(body: &[u8]) -> Result<(), WireError> {
    let stamp: Stamp = decode(body)?;
    if stamp.version != PROTOCOL_VERSION {
        return Err(WireError::Version {
            found: stamp.version,
        });
    }
    Ok(())
}

/// Reads a request, and refuses one written by a version this build does not know.
///
/// # Errors
///
/// Everything [`read`] returns, plus [`WireError::Version`].
#[dacc_derive::doc_anchor(id = "inv-wire-001")]
pub fn read_request<R: Read>(source: &mut R) -> Result<Request, WireError> {
    let body = frame(source)?;
    stamped(&body)?;
    decode(&body)
}

/// Reads a response, and refuses one written by a version this build does not know.
///
/// # Errors
///
/// Everything [`read`] returns, plus [`WireError::Version`].
pub fn read_response<R: Read>(source: &mut R) -> Result<Response, WireError> {
    let body = frame(source)?;
    stamped(&body)?;
    decode(&body)
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
