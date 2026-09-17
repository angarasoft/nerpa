//! State as a directory, with a file for each machine.
//!
//! Format 4 was one file for a whole fleet: every machine's record waited on
//! every other machine's write, and one write cost the fleet's state rather than
//! that machine's. A shard per machine is what lets one outcome reach the disk
//! without the others (RFC 0001 §5.6).
//!
//! # What is in it
//!
//! - `key`, the digest key, written and synced first — loading refuses shards
//!   without it, because a digest under another key compares unequal to every
//!   digest already written and would report every secret as changed.
//! - A shard for each machine, named by a digest of the machine's name so that a
//!   name is a fixed length whatever the machine is called, and `control` for
//!   what lives on no machine.
//! - `format`, naming the version, written last when the directory is made: a
//!   directory without it is a migration that did not finish.
//!
//! Loading knows every name it may see and refuses the run on any other. A
//! directory somebody has put something else into is a directory this cannot
//! reason about, and reading past the stranger would be believing the rest.
//!
//! # What is not here yet
//!
//! Marks, entries, the lock and the migration from format 4, which the same
//! section describes and which come after this.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::address::{Address, Target};
use crate::secret::Key;
use crate::state::State;
use crate::store::{StoreError, StoredRecord};

/// The format this directory is written in.
pub const VERSION: u32 = 5;

/// What a shard holds: the machine it is about, and its records.
#[derive(Serialize, Deserialize)]
struct Shard {
    /// The machine, as it is written in an address, or absent for `control`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    target: Option<String>,
    resources: BTreeMap<String, StoredRecord>,
}

/// What `format` holds.
#[derive(Serialize, Deserialize)]
struct Format {
    version: u32,
}

/// The name of the shard a resource on this machine belongs to.
///
/// A digest rather than the machine's own name: a target may be as long as the
/// grammar allows and may hold characters a filesystem would rather not see,
/// while a shard's name has to be a name. Blake3 rather than the SHA-256 the RFC
/// names — the property wanted here is a fixed length whatever the target, and
/// this workspace already carries blake3 for every other digest it takes.
#[must_use]
pub fn name_of(target: Option<&Target>) -> String {
    match target {
        None => "control".to_owned(),
        Some(target) => blake3::hash(target.as_str().as_bytes())
            .to_hex()
            .to_string(),
    }
}

/// Reads a state directory, or an empty state and a fresh key where there is
/// none yet.
///
/// # Errors
///
/// Returns [`StoreError::Format`] for a name nobody defined, a shard that does
/// not parse, a shard whose machine is not the one its name says, or a directory
/// without its key; [`StoreError::File`] if it could not be read at all; and
/// [`StoreError::Randomness`] if a first run cannot get a key.
pub fn load(directory: &Path) -> Result<(State, Key), StoreError> {
    let refuse = |path: &Path, problem: String| StoreError::Format {
        path: path.display().to_string(),
        problem,
    };

    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        // A first run has no state, and that is not a problem to report.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let key = Key::fresh().map_err(|problem| StoreError::Randomness {
                path: directory.display().to_string(),
                problem: problem.to_string(),
            })?;
            return Ok((State::new(), key));
        }
        Err(error) => {
            return Err(StoreError::File {
                path: directory.display().to_string(),
                problem: error.to_string(),
            });
        }
    };

    let mut shards: Vec<PathBuf> = Vec::new();
    let mut key: Option<Key> = None;
    for entry in entries {
        let entry = entry.map_err(|error| StoreError::File {
            path: directory.display().to_string(),
            problem: error.to_string(),
        })?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        match name.as_str() {
            "format" => {
                let held = read(&path)?;
                let format: Format = serde_json::from_str(&held)
                    .map_err(|error| refuse(&path, error.to_string()))?;
                if format.version != VERSION {
                    return Err(refuse(
                        &path,
                        format!(
                            "written by format version {}, and this is version {VERSION}",
                            format.version
                        ),
                    ));
                }
            }
            "key" => {
                let held = read(&path)?;
                key = Some(Key::from_hex(held.trim()).ok_or_else(|| {
                    refuse(&path, "not thirty-two bytes of hexadecimal".to_owned())
                })?);
            }
            "control" => shards.push(path),
            name if is_a_machine(name) => shards.push(path),
            // Ignored by name, because a write leaves one behind and a report is
            // not state.
            name if is_a_leftover(name) || name.starts_with("report-") => {}
            other => {
                return Err(refuse(
                    &path,
                    format!("{other:?} is not a name this format defines"),
                ));
            }
        }
    }

    let Some(key) = key else {
        return Err(refuse(
            directory,
            "there is no key here, and a digest under another key matches nothing".to_owned(),
        ));
    };

    let mut state = State::new();
    for path in shards {
        held_by(&path, &mut state)?;
    }
    Ok((state, key))
}

/// What a write leaves beside the file it is replacing, until the rename.
fn is_a_leftover(name: &str) -> bool {
    Path::new(name)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("new"))
}

/// Whether this is a shard's name: a digest, and nothing else.
fn is_a_machine(name: &str) -> bool {
    name.len() == 64
        && name
            .chars()
            .all(|ch| ch.is_ascii_hexdigit() && !ch.is_ascii_uppercase())
}

/// Everything one shard holds, added to `state`.
fn held_by(path: &Path, state: &mut State) -> Result<(), StoreError> {
    let refuse = |problem: String| StoreError::Format {
        path: path.display().to_string(),
        problem,
    };
    let held = read(path)?;
    let shard: Shard = serde_json::from_str(&held).map_err(|error| refuse(error.to_string()))?;

    // A shard says which machine it is about, and loading refuses one that has
    // been moved or renamed: its records would then be attributed to a machine
    // nobody wrote them for.
    let target = shard
        .target
        .as_deref()
        .map(|text| Target::new(text).map_err(|error| refuse(format!("{text:?}: {error}"))))
        .transpose()?;
    let expected = name_of(target.as_ref());
    let named = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    if expected != named {
        return Err(refuse(format!(
            "this shard is about {}, which belongs in {expected}",
            target.map_or_else(|| "no machine".to_owned(), |target| target.to_string())
        )));
    }

    for (written, stored) in shard.resources {
        let address: Address = written
            .parse()
            .map_err(|error| refuse(format!("{written:?}: {error}")))?;
        if address.target() != target.as_ref() {
            return Err(refuse(format!(
                "{written} does not live on the machine this shard is about"
            )));
        }
        state.record(address, crate::store::recorded(path, stored)?);
    }
    Ok(())
}

/// Reads one file of the directory.
fn read(path: &Path) -> Result<String, StoreError> {
    fs::read_to_string(path).map_err(|error| StoreError::File {
        path: path.display().to_string(),
        problem: error.to_string(),
    })
}

/// Writes the whole directory: the key first, then every shard, then the format.
///
/// The order is what makes an unfinished directory recognisable. `key` comes
/// first because nothing can be read without it, and `format` last because it is
/// what says the directory is finished.
///
/// # Errors
///
/// Returns [`StoreError::File`] if the directory or any file in it could not be
/// written.
// ENFORCES: INV-STATE-004
pub fn save(directory: &Path, state: &State, key: &Key) -> Result<(), StoreError> {
    let trouble = |path: &Path, error: &std::io::Error| StoreError::File {
        path: path.display().to_string(),
        problem: error.to_string(),
    };

    fs::create_dir_all(directory).map_err(|error| trouble(directory, &error))?;
    write(directory, "key", key.to_hex().as_bytes())?;

    let mut held: BTreeMap<String, Shard> = BTreeMap::new();
    for address in state.addresses() {
        let Some(record) = state.get(address) else {
            continue;
        };
        let shard = held
            .entry(name_of(address.target()))
            .or_insert_with(|| Shard {
                target: address.target().map(ToString::to_string),
                resources: BTreeMap::new(),
            });
        shard
            .resources
            .insert(address.to_string(), crate::store::stored(record));
    }

    for (name, shard) in held {
        let text = serde_json::to_string_pretty(&shard).map_err(|error| StoreError::File {
            path: directory.join(&name).display().to_string(),
            problem: error.to_string(),
        })?;
        write(directory, &name, text.as_bytes())?;
    }

    let format = serde_json::to_string_pretty(&Format { version: VERSION }).map_err(|error| {
        StoreError::File {
            path: directory.join("format").display().to_string(),
            problem: error.to_string(),
        }
    })?;
    write(directory, "format", format.as_bytes())?;

    // The directory itself, so that the names of everything written above
    // survive the machine stopping and not only the process.
    fs::File::open(directory)
        .and_then(|held| held.sync_all())
        .map_err(|error| trouble(directory, &error))
}

/// One file of the directory, written beside and moved into place.
fn write(directory: &Path, name: &str, bytes: &[u8]) -> Result<(), StoreError> {
    let path = directory.join(name);
    let temporary = directory.join(format!("{name}.new"));
    crate::store::durably(&temporary, &path, bytes).map_err(|error| StoreError::File {
        path: path.display().to_string(),
        problem: error.to_string(),
    })
}
