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

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::address::{Address, Target};
use crate::plan::ChangeKind;
use crate::provider::{Afterwards, Attributes};
use crate::secret::Key;
use crate::state::{Mark, Standing, State, Written};
use crate::store::{Lock, StoreError, StoredRecord};

/// The format this directory is written in.
pub const VERSION: u32 = 5;

/// The shard for what lives on no machine, which has no target to name it by.
const CONTROL: &str = "control";

/// What a shard holds: the machine it is about, its records, and the marks of
/// changes that were sent for it.
#[derive(Serialize, Deserialize)]
struct Shard {
    /// The machine, as it is written in an address, or absent for `control`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    target: Option<String>,
    resources: BTreeMap<String, StoredRecord>,
    /// Absent when there are none, so an ordinary shard carries no sign of a
    /// mechanism that did not fire.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    marks: BTreeMap<String, StoredMark>,
}

/// What one mark looks like on disk.
#[derive(Serialize, Deserialize)]
struct StoredMark {
    kind: ChangeKind,
    locator: Attributes,
    /// The settled values that were not secrets. A mark never holds a secret.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    shown: Attributes,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    depends_on: Vec<String>,
    written: u64,
    /// True only for a replacement whose removal half is done.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    past_forgetting: bool,
    afterwards: Afterwards,
    standing: Standing,
    /// The entry this mark was written over, where it was written over one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    replaced: Option<Box<StoredMark>>,
}

/// What a mark looks like on its way to disk.
fn stored_mark(mark: &Mark) -> StoredMark {
    StoredMark {
        kind: mark.kind(),
        locator: mark.locator().clone(),
        shown: mark.shown().clone(),
        depends_on: mark.depends_on().iter().map(ToString::to_string).collect(),
        written: mark.written().seconds(),
        past_forgetting: mark.is_past_forgetting(),
        afterwards: mark.afterwards_said(),
        standing: mark.stands(),
        replaced: mark
            .replaced()
            .map(|replaced| Box::new(stored_mark(replaced))),
    }
}

/// And what it is when it comes back.
fn marked(path: &Path, stored: StoredMark) -> Result<Mark, StoreError> {
    let refuse = |problem: String| StoreError::Format {
        path: path.display().to_string(),
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

    let mut mark = Mark::new(stored.kind, stored.locator, stored.shown)
        .after(depends_on)
        .afterwards(stored.afterwards)
        .standing(stored.standing)
        .written_at(Written::at(stored.written));
    if stored.past_forgetting {
        mark = mark.past_forgetting();
    }
    if let Some(replaced) = stored.replaced {
        mark = mark.replacing(marked(path, *replaced)?);
    }
    Ok(mark)
}

/// What `format` holds.
#[derive(Serialize, Deserialize)]
struct Format {
    version: u32,
    /// The digest of the version 4 file this directory was built from, where it
    /// was built from one.
    ///
    /// It is what lets a migration that stopped after writing this be finished
    /// rather than refused: a version 4 file still at `<state>` is either the one
    /// this came from, and the rest of the migration can be done, or it is
    /// another file, and nothing here knows what to do with it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    from: Option<String>,
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
        None => CONTROL.to_owned(),
        Some(target) => blake3::hash(target.as_str().as_bytes())
            .to_hex()
            .to_string(),
    }
}

/// What a task means to do with state, which decides the lock it takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Taking {
    /// Reading only, as planning does. Several may hold it at once, and none of
    /// them may start while something is writing.
    Reading,
    /// Writing, as applying, releasing and the migration do.
    Writing,
}

/// Opens state: migrates a version 4 file where there is one, takes the lock the
/// task wants, and reads what is there.
///
/// One step rather than three, because the three belong together and forgetting
/// any of them is quiet: a run that does not migrate reads a first run and makes
/// everything a second time, and one that does not lock writes over what another
/// run did not know about.
///
/// The lock the migration takes is released before this one is: they are the
/// same process, one after the other, and nothing else is expected in between.
///
/// # Errors
///
/// Returns [`StoreError::Locked`] if another run holds it, and whatever
/// [`migrate`] and [`load`] refuse.
pub fn opened(state: &Path, taking: Taking) -> Result<(State, Key, Lock), StoreError> {
    migrate(state)?;

    let directory = beside(state);
    fs::create_dir_all(&directory).map_err(|error| StoreError::File {
        path: directory.display().to_string(),
        problem: error.to_string(),
    })?;
    let lock = directory.join("lock");
    let held = match taking {
        Taking::Reading => Lock::shared_at(&lock),
        Taking::Writing => Lock::at(&lock),
    }?;

    let (state, key) = load(&directory)?;
    Ok((state, key, held))
}

/// Where the directory for this state file lives.
///
/// Beside it and never in its place: a directory cannot be renamed onto a file,
/// and renaming the other way round leaves a moment with no state at all, which
/// loads as a first run with a fresh key and nothing recorded.
#[must_use]
pub fn beside(state: &Path) -> PathBuf {
    let mut name = state.as_os_str().to_owned();
    name.push(".d");
    PathBuf::from(name)
}

/// Where the version 4 file is kept once the directory holds what it held.
fn kept(state: &Path) -> PathBuf {
    let mut name = state.as_os_str().to_owned();
    name.push(".v4");
    PathBuf::from(name)
}

/// What is left at the old path: something a version 4 binary refuses by its
/// version rather than reads as a first run.
#[derive(Serialize)]
struct Moved {
    version: u32,
    moved_to: String,
}

/// Moves a version 4 state file into a version 5 directory beside it.
///
/// Does nothing where there is no version 4 file, and finishes a migration that
/// stopped part way rather than starting a second one. Both locks are taken, the
/// version 4 file's first and the directory's second, so that two orders cannot
/// deadlock against each other and neither is left behind by a run that was
/// killed. See RFC 0001 §5.13.
///
/// # Errors
///
/// Returns [`StoreError::Locked`] if a run holds either lock, [`StoreError::File`]
/// if anything could not be read or written, and [`StoreError::Format`] if the
/// version 4 file is not the one an existing directory was built from — that is
/// a file nothing here knows what to do with, and guessing would lose whichever
/// of the two is the real state.
pub fn migrate(state: &Path) -> Result<(), StoreError> {
    let directory = beside(state);
    let refuse = |path: &Path, problem: String| StoreError::Format {
        path: path.display().to_string(),
        problem,
    };

    let Some(bytes) = read_if_there(state)? else {
        // Nothing to migrate: either this is a first run, or the directory is
        // already all there is.
        return Ok(());
    };
    // A version 4 file is the only thing worth migrating. What this run itself
    // left at that path says it has moved, and is not state.
    let stamp: Format =
        serde_json::from_slice(&bytes).map_err(|error| refuse(state, error.to_string()))?;
    if stamp.version != crate::store::FORMAT_VERSION {
        return Ok(());
    }

    let digest = blake3::hash(&bytes).to_hex().to_string();

    // Taken in this order and never the other, so that two of these cannot each
    // hold what the other is waiting for.
    let _version4 = Lock::take(state)?;
    fs::create_dir_all(&directory).map_err(|error| StoreError::File {
        path: directory.display().to_string(),
        problem: error.to_string(),
    })?;
    let _directory = Lock::at(&directory.join("lock"))?;

    match built_from(&directory)? {
        // Finished already, from this very file: what is left is the tail of it.
        Built::FromFile(from) if from == digest => {}
        // Finished, from something else. Which of the two is the state nobody
        // here can say.
        Built::FromFile(from) => {
            return Err(refuse(
                state,
                format!(
                    "this directory was built from a version 4 file of digest {from}, and \
                     this one is {digest}"
                ),
            ));
        }
        // Finished, and not by a migration at all: a directory this run made for
        // itself, which a version 4 file beside it does not belong to.
        Built::OfItsOwn => {
            return Err(refuse(
                state,
                "there is a version 4 file beside a directory that was not built from one"
                    .to_owned(),
            ));
        }
        // A migration that did not get as far as saying it had finished. What it
        // wrote is not to be trusted: it is built again from the file, which is
        // still the state.
        Built::Unfinished => {
            let (held, key) = crate::store::load(state)?;
            remove(&directory)?;
            write_all(&directory, &held, &key, Some(&digest))?;
        }
    }

    finish(state, &directory, &bytes, &digest)
}

/// The tail of a migration: the old file kept, and the old path made into
/// something version 4 refuses.
///
/// Written in this order because `<state>` must never be missing: a version 4
/// binary that found nothing there would start empty and make everything again.
fn finish(state: &Path, directory: &Path, bytes: &[u8], digest: &str) -> Result<(), StoreError> {
    let trouble = |path: &Path, problem: String| StoreError::File {
        path: path.display().to_string(),
        problem,
    };

    let keeping = kept(state);
    let already = read_if_there(&keeping)?
        .is_some_and(|held| blake3::hash(&held).to_hex().to_string() == digest);
    if !already {
        fs::write(&keeping, bytes).map_err(|error| trouble(&keeping, error.to_string()))?;
    }

    let moved = serde_json::to_string_pretty(&Moved {
        version: VERSION,
        moved_to: directory.display().to_string(),
    })
    .map_err(|error| trouble(state, error.to_string()))?;
    let mut temporary = state.as_os_str().to_owned();
    temporary.push(".moved");
    crate::store::durably(PathBuf::from(temporary).as_path(), state, moved.as_bytes())
        .map_err(|error| trouble(state, error.to_string()))
}

/// What is at the directory's path already.
#[derive(Debug)]
enum Built {
    /// Nothing that says it finished: either no directory at all, or one a
    /// migration left before it could say it was done.
    Unfinished,
    /// A finished directory, built from the version 4 file of this digest.
    FromFile(String),
    /// A finished directory that was never built from a version 4 file — one a
    /// run made for itself.
    OfItsOwn,
}

fn built_from(directory: &Path) -> Result<Built, StoreError> {
    let Some(bytes) = read_if_there(&directory.join("format"))? else {
        return Ok(Built::Unfinished);
    };
    let format: Format = serde_json::from_slice(&bytes).map_err(|error| StoreError::Format {
        path: directory.join("format").display().to_string(),
        problem: error.to_string(),
    })?;
    Ok(match format.from {
        Some(from) => Built::FromFile(from),
        None => Built::OfItsOwn,
    })
}

/// The bytes of a file, or nothing where there is no file.
fn read_if_there(path: &Path) -> Result<Option<Vec<u8>>, StoreError> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(StoreError::File {
            path: path.display().to_string(),
            problem: error.to_string(),
        }),
    }
}

/// Takes away a directory that a migration did not finish.
fn remove(directory: &Path) -> Result<(), StoreError> {
    for entry in fs::read_dir(directory).map_err(|error| StoreError::File {
        path: directory.display().to_string(),
        problem: error.to_string(),
    })? {
        let entry = entry.map_err(|error| StoreError::File {
            path: directory.display().to_string(),
            problem: error.to_string(),
        })?;
        // Everything but the lock this run is holding.
        if entry.file_name() == "lock" {
            continue;
        }
        fs::remove_file(entry.path()).map_err(|error| StoreError::File {
            path: entry.path().display().to_string(),
            problem: error.to_string(),
        })?;
    }
    Ok(())
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
            "lock" => {}
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
            CONTROL => shards.push(path),
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
        // A directory holding nothing but the lock a run has just taken is a
        // first run: the lock had to exist before there was anything to lock.
        if shards.is_empty() {
            let key = Key::fresh().map_err(|problem| StoreError::Randomness {
                path: directory.display().to_string(),
                problem: problem.to_string(),
            })?;
            return Ok((State::new(), key));
        }
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

/// The shards already in the directory: one machine's digest, or `control`.
///
/// Nothing else is named here. `key`, `format` and `lock` belong to the
/// directory rather than to a machine, and a leftover `.new` is somebody's
/// unfinished write.
fn beside_them(directory: &Path) -> Result<Vec<String>, StoreError> {
    let trouble = |error: &std::io::Error| StoreError::File {
        path: directory.display().to_string(),
        problem: error.to_string(),
    };
    let mut names = Vec::new();
    for entry in fs::read_dir(directory).map_err(|error| trouble(&error))? {
        let name = entry
            .map_err(|error| trouble(&error))?
            .file_name()
            .to_string_lossy()
            .into_owned();
        if name == CONTROL || is_a_machine(&name) {
            names.push(name);
        }
    }
    Ok(names)
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
        // A path with no file name matches nothing, which is what the check
        // below wants to hear.
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    if expected != named {
        return Err(refuse(format!(
            "this shard is about {}, which belongs in {expected}",
            target.map_or_else(|| "no machine".to_owned(), |target| target.to_string())
        )));
    }

    for (written, stored) in shard.resources {
        let address = belonging(&written, target.as_ref(), path)?;
        state.record(address, crate::store::recorded(path, stored)?);
    }
    for (written, stored) in shard.marks {
        let address = belonging(&written, target.as_ref(), path)?;
        state.mark(address, marked(path, stored)?);
    }
    Ok(())
}

/// An address from a shard, checked against the machine that shard is about.
fn belonging(written: &str, target: Option<&Target>, path: &Path) -> Result<Address, StoreError> {
    let refuse = |problem: String| StoreError::Format {
        path: path.display().to_string(),
        problem,
    };
    let address: Address = written
        .parse()
        .map_err(|error| refuse(format!("{written:?}: {error}")))?;
    if address.target() != target {
        return Err(refuse(format!(
            "{written} does not live on the machine this shard is about"
        )));
    }
    Ok(address)
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
#[dacc_derive::doc_anchor(id = "inv-state-004-1")]
pub fn save(directory: &Path, state: &State, key: &Key) -> Result<(), StoreError> {
    write_all(directory, state, key, None)
}

/// The same, saying what the directory was built from.
fn write_all(
    directory: &Path,
    state: &State,
    key: &Key,
    from: Option<&str>,
) -> Result<(), StoreError> {
    let trouble = |path: &Path, error: &std::io::Error| StoreError::File {
        path: path.display().to_string(),
        problem: error.to_string(),
    };

    fs::create_dir_all(directory).map_err(|error| trouble(directory, &error))?;
    write(directory, "key", key.to_hex().as_bytes())?;

    let mut held: BTreeMap<String, Shard> = BTreeMap::new();
    let shard_for = |held: &mut BTreeMap<String, Shard>, address: &Address| {
        held.entry(name_of(address.target()))
            .or_insert_with(|| Shard {
                target: address.target().map(ToString::to_string),
                resources: BTreeMap::new(),
                marks: BTreeMap::new(),
            });
        name_of(address.target())
    };
    for address in state.addresses() {
        let Some(record) = state.get(address) else {
            continue;
        };
        let name = shard_for(&mut held, address);
        if let Some(shard) = held.get_mut(&name) {
            shard
                .resources
                .insert(address.to_string(), crate::store::stored(record));
        }
    }
    // A machine with a change in flight and nothing recorded still has a shard:
    // the mark is what says that machine was being acted on.
    for (address, mark) in state.marks() {
        let name = shard_for(&mut held, address);
        if let Some(shard) = held.get_mut(&name) {
            shard.marks.insert(address.to_string(), stored_mark(mark));
        }
    }

    let written: BTreeSet<String> = held.keys().cloned().collect();
    for (name, shard) in held {
        let text = serde_json::to_string_pretty(&shard).map_err(|error| StoreError::File {
            path: directory.join(&name).display().to_string(),
            problem: error.to_string(),
        })?;
        write(directory, &name, text.as_bytes())?;
    }

    // A machine state has nothing for has no shard to write — and the file from
    // when it had something would sit there saying otherwise. So the last thing
    // on a machine going away has to take that machine's shard with it, or a
    // resource removed on the last machine it lived on is remembered for ever
    // and every later plan acts on it. Taken away after the shards that do have
    // something are written, and made durable by the directory's own sync
    // below, so a crash in the middle leaves a state that was true earlier
    // rather than one that was never true.
    for name in beside_them(directory)? {
        if !written.contains(&name) {
            let path = directory.join(&name);
            fs::remove_file(&path).map_err(|error| trouble(&path, &error))?;
        }
    }

    let format = serde_json::to_string_pretty(&Format {
        version: VERSION,
        from: from.map(ToOwned::to_owned),
    })
    .map_err(|error| StoreError::File {
        path: directory.join("format").display().to_string(),
        problem: error.to_string(),
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
