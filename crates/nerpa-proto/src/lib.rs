//! The wire between the engine and an executor on a machine.
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

use std::collections::BTreeMap;
use std::io::{Read, Write};

use serde::{Deserialize, Serialize};

/// The version every frame is stamped with.
///
/// Bumped when the meaning of anything below changes. An executor that reads a
/// version it does not know refuses the frame; it does not attempt a subset.
pub const PROTOCOL_VERSION: u32 = 1;

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
    /// What do these paths look like now?
    ///
    /// A batch, because the whole reason this wire exists is that one journey to
    /// a machine should answer for everything on it.
    Look {
        /// Everything on this machine worth looking at, in one go.
        paths: Vec<String>,
    },
    /// Make these so, in the order given.
    Perform {
        /// Carried out in the order given, and stopping at the first failure.
        operations: Vec<Operation>,
    },
    /// What did you last apply, as one number?
    ///
    /// The cheapest of the three tiers in decision 0011: a node whose digest
    /// matches what state remembers is skipped whole.
    Fingerprint,
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
    /// Gone.
    Remove {
        /// What to take away.
        path: String,
    },
}

impl Operation {
    /// Where this operation acts.
    #[must_use]
    pub fn path(&self) -> &str {
        match self {
            Operation::Directory { path, .. }
            | Operation::File { path, .. }
            | Operation::Remove { path } => path,
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
    /// What was found, by path. A path that is absent does not exist.
    Found {
        /// Only what is there.
        entries: BTreeMap<String, Entry>,
    },
    /// Everything asked for was done, and this is what those paths became.
    Done {
        /// Read back rather than echoed, so that writing nothing is noticed.
        entries: BTreeMap<String, Entry>,
    },
    /// One number standing for everything the executor last applied.
    Fingerprint {
        /// Hexadecimal digest.
        digest: String,
    },
    /// It could not be done, and this is as far as it got.
    Failed {
        /// Where it stopped.
        path: String,
        /// What the machine said.
        problem: String,
        /// How many operations had already succeeded, so the engine knows what
        /// happened rather than assuming nothing did.
        completed: usize,
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
