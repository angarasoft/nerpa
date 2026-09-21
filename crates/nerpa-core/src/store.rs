//! Writing state down, and not losing it.
//!
//! State is the only thing Nerpa cannot recover by looking: what it made, and
//! what that depended on. So it is written before anything else believes the run
//! finished, and it is written whole — a half-written state file is worse than
//! no state file, because it claims things that are not true.
//!
//! # The format
//!
//! JSON, and readable on purpose. A state file gets opened by a person with
//! `jq` at some point, usually a bad one. Addresses are written as text rather
//! than as structures, which works because parsing and printing an address round
//! trip exactly — the property `INV-ADDR-003` exists for.
//!
//! ```text
//!   { "version": 3,
//!     "digest_key": "3f9a…",
//!     "resources": {
//!       "os:file.motd@web-01": {
//!         "attributes": { "path": "/etc/motd", "content": "hello\n" },
//!         "secrets": { "token": "9c41…" },
//!         "depends_on": [] } } }
//! ```
//!
//! Secrets are in a map of their own rather than mixed in among the attributes,
//! and this is deliberate: a person running `jq .resources[].attributes` over a
//! state file can see that no value in it is a credential, which is a guarantee
//! worth being able to check by looking. `digest_key` is what those digests were
//! taken under — see [`crate::secret`] for what that does and does not protect.
//!
//! # The lock
//!
//! Two runs against one state file would each write what the other did not know
//! about. The lock is a file created exclusively, and released by a guard whose
//! destructor runs on unwind — which is why the release profile must never set
//! `panic = "abort"`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::address::Address;
use crate::provider::Attributes;
use crate::secret::{Digest, Key, Recorded};
use crate::state::{Record, Recording, State, Written};

/// The format version written into every state file.
///
/// Read back and refused if it is not understood. A state file from a later
/// version is not something to guess at.
pub const FORMAT_VERSION: u32 = 4;

/// Why state could not be read or written.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum StoreError {
    /// The file could not be opened, read or written.
    #[error("{path}: {problem}")]
    File {
        /// Which file.
        path: String,
        /// What the operating system said.
        problem: String,
    },
    /// The file is not state this version understands.
    #[error("{path}: {problem}")]
    Format {
        /// Which file.
        path: String,
        /// What was wrong with it.
        problem: String,
    },
    /// The operating system would not supply randomness for a digest key.
    #[error("{path}: {problem}")]
    Randomness {
        /// Which file was being started.
        path: String,
        /// What went wrong.
        problem: String,
    },
    /// Another run holds the lock.
    #[error("{path} is held by {holder}; that run has it until it ends")]
    Locked {
        /// The lock file that is in the way.
        path: String,
        /// What the file says about who took it.
        holder: String,
    },
}

/// Just enough of a state file to say which version wrote it.
#[derive(Deserialize)]
struct Stamp {
    version: u32,
}

#[derive(Serialize, Deserialize)]
struct Document {
    version: u32,
    /// Hexadecimal, and the reason digests in this file compare only against
    /// each other.
    digest_key: String,
    resources: BTreeMap<String, StoredRecord>,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct StoredRecord {
    pub(crate) attributes: Attributes,
    /// Digests of the attributes that were secret. Absent when there were none,
    /// so that an ordinary state file is not littered with empty maps.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) secrets: BTreeMap<String, Digest>,
    pub(crate) depends_on: Vec<String>,
    /// Reactions this resource owes: what roused it, and was not paid before
    /// the run stopped. Absent when there are none, so an ordinary state file
    /// carries no sign of a mechanism that did not fire.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) owed: Vec<String>,
    /// Seconds since the epoch, and the only reason a clock is read at all: how
    /// old what we know about a machine is, when that machine will not answer.
    /// See decision 0020.
    pub(crate) written: u64,
}

/// What one record looks like on disk.
pub(crate) fn stored(record: &Record) -> StoredRecord {
    StoredRecord {
        attributes: record.locator(),
        secrets: record
            .attributes()
            .iter()
            .filter_map(|(name, recorded)| match recorded {
                Recorded::Secret(digest) => Some((name.clone(), *digest)),
                Recorded::Plain(_) => None,
            })
            .collect(),
        depends_on: record
            .depends_on()
            .iter()
            .map(ToString::to_string)
            .collect(),
        owed: record.owed().iter().map(ToString::to_string).collect(),
        written: record.written().seconds(),
    }
}

/// And what it is when it comes back.
///
/// # Errors
///
/// Returns [`StoreError::Format`] if an address it names is not one this model
/// accepts: a file naming something unaddressable is a file this cannot act on,
/// and reading past it would drop what it said.
pub(crate) fn recorded(where_from: &Path, stored: StoredRecord) -> Result<Record, StoreError> {
    let refuse = |problem: String| StoreError::Format {
        path: where_from.display().to_string(),
        problem,
    };
    let depends_on = stored
        .depends_on
        .iter()
        .map(|text| {
            text.parse::<Address>()
                .map_err(|error| refuse(format!("{text:?}: {error}")))
        })
        .collect::<Result<_, _>>()?;
    let owed = stored
        .owed
        .iter()
        .map(|text| {
            text.parse::<Address>()
                .map_err(|error| refuse(format!("{text:?}: {error}")))
        })
        .collect::<Result<_, _>>()?;
    let mut attributes: Recording = Recorded::all(stored.attributes);
    for (name, digest) in stored.secrets {
        attributes.insert(name, Recorded::Secret(digest));
    }
    Ok(Record::written_at(attributes, depends_on, Written::at(stored.written)).owing(owed))
}

/// Holds the lock for as long as it exists.
///
/// The file is the lock's name; the `flock` on this open file is the lock. It is
/// dropped by closing, which the kernel does whatever ends the process.
#[derive(Debug)]
pub struct Lock {
    /// Never read: what it is for is being open. The `flock` lives as long as
    /// this descriptor does, and closing it is what releases the lock — so the
    /// value is the lock, and reading it would say nothing.
    #[allow(dead_code, reason = "held open for as long as the lock is")]
    held: fs::File,
}

impl Lock {
    /// Takes the lock beside `state` for a run that writes, or reports who has
    /// it.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Locked`] if another run holds it, and
    /// [`StoreError::File`] if the file could not be opened at all.
    pub fn take(state: &Path) -> Result<Self, StoreError> {
        Self::held(state, rustix::fs::FlockOperation::NonBlockingLockExclusive)
    }

    /// Takes it for a run that only reads, which several may hold at once.
    ///
    /// Planning takes it this way: it must not read a live run's marks as
    /// though the run had ended, and it has nothing to write. Two plans can run
    /// side by side; an apply cannot start while either is reading.
    ///
    /// # Errors
    ///
    /// The same as [`Lock::take`].
    pub fn shared(state: &Path) -> Result<Self, StoreError> {
        Self::held(state, rustix::fs::FlockOperation::NonBlockingLockShared)
    }

    /// The lock file, opened and held.
    ///
    /// Created if it is not there and never removed. What makes it a lock is the
    /// `flock` on the open file, which the kernel releases when the last
    /// descriptor closes — so a run that was killed leaves the file behind and
    /// no lock, where before it left a file that refused every later run.
    fn held(state: &Path, how: rustix::fs::FlockOperation) -> Result<Self, StoreError> {
        Self::exactly(&lock_path(state), how)
    }

    /// Takes the lock that *is* this path, rather than one beside it.
    ///
    /// The state directory keeps its lock inside itself, where a name beside it
    /// would be a name the directory does not define.
    ///
    /// # Errors
    ///
    /// The same as [`Lock::take`].
    pub fn at(path: &Path) -> Result<Self, StoreError> {
        Self::exactly(path, rustix::fs::FlockOperation::NonBlockingLockExclusive)
    }

    fn exactly(path: &Path, how: rustix::fs::FlockOperation) -> Result<Self, StoreError> {
        let trouble = |problem: String| StoreError::File {
            path: path.display().to_string(),
            problem,
        };
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .map_err(|error| trouble(error.to_string()))?;

        match rustix::fs::flock(&file, how) {
            Ok(()) => {}
            Err(rustix::io::Errno::WOULDBLOCK) => {
                // Who holds it, so the refusal can say. A lock that names nobody
                // leaves a person one option — delete the file — which does
                // nothing now and was the dangerous thing before.
                let holder = fs::read_to_string(path)
                    .ok()
                    .filter(|written| !written.trim().is_empty())
                    .unwrap_or_else(|| "a run that did not say which".to_owned());
                return Err(StoreError::Locked {
                    path: path.display().to_string(),
                    holder: holder.trim().to_owned(),
                });
            }
            Err(error) => return Err(trouble(error.to_string())),
        }

        if how == rustix::fs::FlockOperation::NonBlockingLockExclusive {
            use std::io::Write as _;
            let _ = file.set_len(0);
            let _ = write!(
                &file,
                "pid {} since {}",
                std::process::id(),
                Written::now().seconds()
            );
        }
        Ok(Self { held: file })
    }
}

fn lock_path(state: &Path) -> PathBuf {
    let mut name = state.as_os_str().to_owned();
    name.push(".lock");
    PathBuf::from(name)
}

/// Reads state and the key its digests were taken under, or an empty state and a
/// fresh key if the file is not there yet.
///
/// The key comes back with the state because the two are only meaningful
/// together: a digest taken under a different key compares unequal to every
/// digest already written, which would report every secret as changed.
///
/// # Errors
///
/// Returns [`StoreError`] if the file exists but cannot be read, is not JSON, or
/// was written by a version this one does not understand, and
/// [`StoreError::Randomness`] if a first run cannot get a key at all.
pub fn load(path: &Path) -> Result<(State, Key), StoreError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        // A first run has no state, and that is not a problem to report.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let key = Key::fresh().map_err(|problem| StoreError::Randomness {
                path: path.display().to_string(),
                problem: problem.to_string(),
            })?;
            return Ok((State::new(), key));
        }
        Err(error) => {
            return Err(StoreError::File {
                path: path.display().to_string(),
                problem: error.to_string(),
            });
        }
    };

    // The version is read on its own first, and the file parsed afterwards. A
    // state file from another version will be missing fields this one requires,
    // and complaining about a missing field tells whoever is reading the message
    // nothing about what actually happened to them.
    let stamp: Stamp = serde_json::from_str(&text).map_err(|error| StoreError::Format {
        path: path.display().to_string(),
        problem: error.to_string(),
    })?;

    if stamp.version != FORMAT_VERSION {
        return Err(StoreError::Format {
            path: path.display().to_string(),
            problem: format!(
                "written by format version {}, and this is version {FORMAT_VERSION}",
                stamp.version
            ),
        });
    }

    let document: Document = serde_json::from_str(&text).map_err(|error| StoreError::Format {
        path: path.display().to_string(),
        problem: error.to_string(),
    })?;

    let mut state = State::new();
    for (written, record) in document.resources {
        let address: Address = written.parse().map_err(|error| StoreError::Format {
            path: path.display().to_string(),
            problem: format!("{written:?}: {error}"),
        })?;
        let depends_on = record
            .depends_on
            .iter()
            .map(|text| {
                text.parse::<Address>().map_err(|error| StoreError::Format {
                    path: path.display().to_string(),
                    problem: format!("{text:?}: {error}"),
                })
            })
            .collect::<Result<_, _>>()?;
        let mut attributes: Recording = Recorded::all(record.attributes);
        for (name, digest) in record.secrets {
            attributes.insert(name, Recorded::Secret(digest));
        }
        let owed = record
            .owed
            .iter()
            .map(|text| {
                text.parse::<Address>().map_err(|error| StoreError::Format {
                    path: path.display().to_string(),
                    problem: format!("{text:?}: {error}"),
                })
            })
            .collect::<Result<_, _>>()?;
        state.record(
            address,
            Record::written_at(attributes, depends_on, Written::at(record.written)).owing(owed),
        );
    }

    let key = Key::from_hex(&document.digest_key).ok_or_else(|| StoreError::Format {
        path: path.display().to_string(),
        problem: "digest_key is not thirty-two bytes of hexadecimal".to_owned(),
    })?;
    Ok((state, key))
}

/// Writes state, whole or not at all.
///
/// # Errors
///
/// Returns [`StoreError::File`] if the file could not be written or moved into
/// place.
// ENFORCES: INV-STATE-004
pub fn save(path: &Path, state: &State, key: &Key) -> Result<(), StoreError> {
    let document = Document {
        version: FORMAT_VERSION,
        digest_key: key.to_hex(),
        resources: state
            .addresses()
            .filter_map(|address| {
                let record = state.get(address)?;
                Some((
                    address.to_string(),
                    StoredRecord {
                        attributes: record.locator(),
                        secrets: record
                            .attributes()
                            .iter()
                            .filter_map(|(name, recorded)| match recorded {
                                Recorded::Secret(digest) => Some((name.clone(), *digest)),
                                Recorded::Plain(_) => None,
                            })
                            .collect(),
                        depends_on: record
                            .depends_on()
                            .iter()
                            .map(ToString::to_string)
                            .collect(),
                        owed: record.owed().iter().map(ToString::to_string).collect(),
                        written: record.written().seconds(),
                    },
                ))
            })
            .collect(),
    };

    let text = serde_json::to_string_pretty(&document).map_err(|error| StoreError::File {
        path: path.display().to_string(),
        problem: error.to_string(),
    })?;

    // Written beside the target and moved into place, so that a run interrupted
    // mid-write leaves the previous state intact rather than half of two.
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(".new");
    let temporary = PathBuf::from(temporary);

    durably(&temporary, path, text.as_bytes()).map_err(|error| StoreError::File {
        path: path.display().to_string(),
        problem: error.to_string(),
    })
}

/// Writes bytes and puts them under a name, so that both survive the machine
/// stopping rather than only the process.
///
/// A write followed by a rename is atomic against a process that dies: a reader
/// sees the old file or the new one. It is not atomic against a machine that
/// loses power, because neither the bytes nor the new directory entry are on the
/// disk yet — the file can come back empty, or under its temporary name, or not
/// at all. Three calls make the difference, and this module's promise of "whole
/// or not at all" was worth two of them less than it said.
// ENFORCES: INV-STATE-004
pub(crate) fn durably(temporary: &Path, path: &Path, bytes: &[u8]) -> Result<(), std::io::Error> {
    use std::io::Write as _;

    let mut file = fs::File::create(temporary)?;
    file.write_all(bytes)?;
    // The bytes.
    file.sync_all()?;
    drop(file);

    fs::rename(temporary, path)?;
    // And the name. A rename changes a directory, and a directory is a file
    // that has to be written down like any other.
    fs::File::open(directory_of(path))?.sync_all()
}

/// The directory a path sits in, which for a bare name is the current one.
pub(crate) fn directory_of(path: &Path) -> &Path {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    }
}
