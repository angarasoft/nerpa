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
//!   { "version": 1,
//!     "resources": {
//!       "os:file.motd@web-01": {
//!         "attributes": { "path": "/etc/motd", "content": "hello\n" },
//!         "depends_on": [] } } }
//! ```
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
use crate::state::{Record, State};

/// The format version written into every state file.
///
/// Read back and refused if it is not understood. A state file from a later
/// version is not something to guess at.
pub const FORMAT_VERSION: u32 = 1;

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
    /// Another run holds the lock.
    #[error("{path} is locked by another run; remove it if you are sure there is not one")]
    Locked {
        /// The lock file that is in the way.
        path: String,
    },
}

#[derive(Serialize, Deserialize)]
struct Document {
    version: u32,
    resources: BTreeMap<String, StoredRecord>,
}

#[derive(Serialize, Deserialize)]
struct StoredRecord {
    attributes: Attributes,
    depends_on: Vec<String>,
}

/// Holds the lock for as long as it exists.
#[derive(Debug)]
pub struct Lock {
    path: PathBuf,
}

impl Lock {
    /// Takes the lock beside `state`, or reports who has it.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Locked`] if another run holds it, and
    /// [`StoreError::File`] if it could not be created at all.
    pub fn take(state: &Path) -> Result<Self, StoreError> {
        let path = lock_path(state);
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(_) => Ok(Self { path }),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                Err(StoreError::Locked {
                    path: path.display().to_string(),
                })
            }
            Err(error) => Err(StoreError::File {
                path: path.display().to_string(),
                problem: error.to_string(),
            }),
        }
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        // Best effort on the way out. A lock left behind is an inconvenience; a
        // panic while unwinding would be worse.
        let _ = fs::remove_file(&self.path);
    }
}

fn lock_path(state: &Path) -> PathBuf {
    let mut name = state.as_os_str().to_owned();
    name.push(".lock");
    PathBuf::from(name)
}

/// Reads state, or an empty one if the file is not there yet.
///
/// # Errors
///
/// Returns [`StoreError`] if the file exists but cannot be read, is not JSON, or
/// was written by a version this one does not understand.
pub fn load(path: &Path) -> Result<State, StoreError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        // A first run has no state, and that is not a problem to report.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(State::new()),
        Err(error) => {
            return Err(StoreError::File {
                path: path.display().to_string(),
                problem: error.to_string(),
            });
        }
    };

    let document: Document = serde_json::from_str(&text).map_err(|error| StoreError::Format {
        path: path.display().to_string(),
        problem: error.to_string(),
    })?;

    if document.version != FORMAT_VERSION {
        return Err(StoreError::Format {
            path: path.display().to_string(),
            problem: format!(
                "written by format version {}, and this is version {FORMAT_VERSION}",
                document.version
            ),
        });
    }

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
        state.record(address, Record::new(record.attributes, depends_on));
    }
    Ok(state)
}

/// Writes state, whole or not at all.
///
/// # Errors
///
/// Returns [`StoreError::File`] if the file could not be written or moved into
/// place.
// ENFORCES: INV-STATE-004
pub fn save(path: &Path, state: &State) -> Result<(), StoreError> {
    let document = Document {
        version: FORMAT_VERSION,
        resources: state
            .addresses()
            .filter_map(|address| {
                let record = state.get(address)?;
                Some((
                    address.to_string(),
                    StoredRecord {
                        attributes: record.attributes().clone(),
                        depends_on: record
                            .depends_on()
                            .iter()
                            .map(ToString::to_string)
                            .collect(),
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

    let write = fs::write(&temporary, text).and_then(|()| fs::rename(&temporary, path));
    write.map_err(|error| StoreError::File {
        path: path.display().to_string(),
        problem: error.to_string(),
    })
}
