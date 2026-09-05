//! Files and directories on a machine.
//!
//! The first provider, and deliberately the smallest useful one. It owns two
//! kinds — `os:file` and `os:directory` — and it runs where Nerpa runs, with no
//! transport in between. Packages, services, users and sysctls need privileges
//! and a real system underneath, so they arrive with the executor rather than
//! here.
//!
//! # How the pieces fit
//!
//! ```text
//!   Os ─── read  ─── what is on disk now, nothing, or a refusal to guess
//!      └── apply ─── writes it, and reports back what it became
//!            │
//!            └── Backend ─── Disk, or Memory for the fake
//! ```
//!
//! # Correctness
//!
//! `apply` reports what the resource *became* by reading it back, not by
//! repeating what it was told. A provider that echoes its input cannot notice
//! that it wrote nothing, and the next plan would be empty for the wrong reason.
//!
//! Reading answers one of **three** things, never two: what is there, that
//! nothing is there, or that it could not tell. The third is the one that
//! matters. A provider that reports a file it may not open as absent makes the
//! plan propose creating it and the apply overwrite it — losing the contents and
//! resetting the mode — which is the silent wrongness this project exists to
//! avoid, committed by the tool itself. `ProviderError::Unreachable` says the
//! same thing about machines and says why.
//!
//! Unix only, and not apologetically: file modes are the whole point of the
//! `mode` attribute, and a portable abstraction over them would describe
//! something nobody runs.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use nerpa_core::address::{Address, Kind, Provider as ProviderName};
use nerpa_core::plan::ChangeKind;
use nerpa_core::provider::{Attributes, Provider, ProviderError, Reading};
use nerpa_core::schema::Schema;
use nerpa_core::value::Value;

/// Files and directories.
#[derive(Debug)]
pub struct Os {
    name: ProviderName,
    backend: Backend,
}

/// Where the provider does its work.
#[derive(Debug)]
enum Backend {
    /// The machine this is running on.
    Disk,
    /// Nothing at all, for tests.
    Memory(RefCell<BTreeMap<PathBuf, Entry>>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Entry {
    Directory {
        mode: u32,
    },
    File {
        content: String,
        mode: u32,
    },
    /// A pointer to somewhere else. Not a kind this provider manages, and
    /// reported rather than followed: reading through it would describe one
    /// object and writing through it would change another.
    Symlink {
        target: String,
    },
}

const DEFAULT_FILE_MODE: u32 = 0o644;
const DEFAULT_DIRECTORY_MODE: u32 = 0o755;

impl Os {
    /// A provider that works on the machine it is running on.
    ///
    /// # Panics
    ///
    /// Panics if `os` is somehow not a valid provider name, which would mean the
    /// address grammar had changed underneath this crate.
    #[must_use]
    #[allow(
        clippy::expect_used,
        reason = "a literal that stopped parsing is a bug here"
    )]
    pub fn new() -> Self {
        Self {
            name: ProviderName::new("os").expect("`os` is a valid provider name"),
            backend: Backend::Disk,
        }
    }

    /// What is at a path: something, nothing, or a refusal to say.
    fn entry(&self, path: &Path) -> Result<Option<Entry>, String> {
        match &self.backend {
            Backend::Memory(entries) => Ok(entries.borrow().get(path).cloned()),
            Backend::Disk => read_from_disk(path),
        }
    }

    /// The same, with the address attached so a failure names the resource.
    fn entry_for(&self, address: &Address, path: &Path) -> Result<Option<Entry>, ProviderError> {
        self.entry(path).map_err(|problem| ProviderError::World {
            address: address.clone(),
            problem,
        })
    }

    fn store(&self, path: &Path, entry: Entry) -> Result<(), std::io::Error> {
        match &self.backend {
            Backend::Memory(entries) => {
                entries.borrow_mut().insert(path.to_owned(), entry);
                Ok(())
            }
            Backend::Disk => write_to_disk(path, &entry),
        }
    }
}

impl Default for Os {
    fn default() -> Self {
        Self::new()
    }
}

// ENFORCES: INV-READ-001
fn read_from_disk(path: &Path) -> Result<Option<Entry>, String> {
    let shown = path.display();
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        // The one error that means what it says.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("{shown}: {error}")),
    };

    let kind = metadata.file_type();
    // Asked before anything else, because `symlink_metadata` describes the link
    // and reading the path describes its target. Answering with the mode of one
    // and the contents of the other would be a record of an object that does not
    // exist.
    if kind.is_symlink() {
        let target = fs::read_link(path).map_err(|error| format!("{shown}: {error}"))?;
        return Ok(Some(Entry::Symlink {
            target: target.display().to_string(),
        }));
    }

    let mode = metadata.permissions().mode() & 0o7777;
    if kind.is_dir() {
        return Ok(Some(Entry::Directory { mode }));
    }
    if !kind.is_file() {
        return Err(format!(
            "{shown} is neither a file nor a directory, and this provider has no \
             way to describe what it is"
        ));
    }

    match fs::read_to_string(path) {
        Ok(content) => Ok(Some(Entry::File { content, mode })),
        Err(error) if error.kind() == std::io::ErrorKind::InvalidData => Err(format!(
            "{shown} is not valid UTF-8, and an attribute cannot hold bytes yet"
        )),
        Err(error) => Err(format!("{shown}: {error}")),
    }
}

fn write_to_disk(path: &Path, entry: &Entry) -> Result<(), std::io::Error> {
    match entry {
        Entry::Directory { mode } => {
            fs::create_dir_all(path)?;
            fs::set_permissions(path, fs::Permissions::from_mode(*mode))
        }
        Entry::File { content, mode } => {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            // A symlink here points at a file the declaration does not name, and
            // writing through it would change that one instead. Removing a
            // pointer destroys nothing; the plan showed this path becoming a
            // file, and this is that.
            if fs::symlink_metadata(path).is_ok_and(|found| found.file_type().is_symlink()) {
                fs::remove_file(path)?;
            }
            fs::write(path, content)?;
            fs::set_permissions(path, fs::Permissions::from_mode(*mode))
        }
        Entry::Symlink { target } => Err(std::io::Error::other(format!(
            "{} is a symlink to {target}, and this provider does not make symlinks",
            path.display()
        ))),
    }
}

fn text(attributes: &Attributes, name: &str) -> Option<String> {
    match attributes.get(name)? {
        Value::Text(text) => Some(text.clone()),
        _ => None,
    }
}

/// Reads a mode written the way anybody writes one: `"0644"`.
fn mode(attributes: &Attributes, address: &Address, default: u32) -> Result<u32, ProviderError> {
    let Some(written) = text(attributes, "mode") else {
        return Ok(default);
    };
    u32::from_str_radix(written.trim_start_matches("0o"), 8).map_err(|_| {
        ProviderError::Declaration {
            address: address.clone(),
            problem: format!("mode {written:?} is not an octal file mode such as \"0644\""),
        }
    })
}

fn required_path(attributes: &Attributes, address: &Address) -> Result<PathBuf, ProviderError> {
    text(attributes, "path")
        .map(PathBuf::from)
        .ok_or_else(|| ProviderError::Declaration {
            address: address.clone(),
            problem: "needs a `path`".to_owned(),
        })
}

fn as_attributes(path: &Path, entry: &Entry) -> Attributes {
    let mut attributes = Attributes::new();
    attributes.insert("path".to_owned(), Value::Text(path.display().to_string()));
    match entry {
        Entry::Directory { mode } => {
            attributes.insert("mode".to_owned(), Value::Text(format!("{mode:04o}")));
        }
        Entry::File { content, mode } => {
            attributes.insert("content".to_owned(), Value::Text(content.clone()));
            attributes.insert("mode".to_owned(), Value::Text(format!("{mode:04o}")));
        }
        Entry::Symlink { target } => {
            attributes.insert("symlink_to".to_owned(), Value::Text(target.clone()));
        }
    }
    attributes
}

impl Provider for Os {
    fn name(&self) -> &ProviderName {
        &self.name
    }

    /// A path is the resource's identity on disk. Changing it does not move the
    /// old one; it makes a different one and abandons the first.
    fn schema(&self) -> Schema {
        #[allow(
            clippy::expect_used,
            reason = "literals that stopped parsing are a bug here"
        )]
        let kind = |name: &str| Kind::new(name).expect("a valid kind");
        Schema::new()
            .immutable(self.name.clone(), kind("file"), "path")
            .immutable(self.name.clone(), kind("directory"), "path")
    }

    /// Reads the batch one path at a time, because on this machine there is
    /// nothing to save: `stat` is `stat` whether it is asked for once or ten
    /// times. The batch exists for the version that has to cross a network, and
    /// this one keeps the shape so that version can slot underneath unchanged.
    fn read(&self, batch: &[Reading]) -> Result<BTreeMap<Address, Attributes>, ProviderError> {
        let mut found = BTreeMap::new();

        for reading in batch {
            let address = reading.address();
            match address.kind().as_str() {
                "file" | "directory" => {}
                _ => {
                    return Err(ProviderError::UnknownKind {
                        provider: self.name.clone(),
                        address: address.clone(),
                    });
                }
            }

            // No settled path means the declaration is waiting on something that
            // has not happened. Nothing to look at, and nothing to guess.
            let Some(path) = text(reading.locator(), "path").map(PathBuf::from) else {
                continue;
            };

            if let Some(entry) = self.entry_for(address, &path)? {
                found.insert(address.clone(), as_attributes(&path, &entry));
            }
        }

        Ok(found)
    }

    fn apply(
        &self,
        address: &Address,
        _kind: ChangeKind,
        attributes: &Attributes,
    ) -> Result<Attributes, ProviderError> {
        let path = required_path(attributes, address)?;
        let entry = match address.kind().as_str() {
            "directory" => Entry::Directory {
                mode: mode(attributes, address, DEFAULT_DIRECTORY_MODE)?,
            },
            "file" => Entry::File {
                content: text(attributes, "content").unwrap_or_default(),
                mode: mode(attributes, address, DEFAULT_FILE_MODE)?,
            },
            _ => {
                return Err(ProviderError::UnknownKind {
                    provider: self.name.clone(),
                    address: address.clone(),
                });
            }
        };

        self.store(&path, entry)
            .map_err(|error| ProviderError::World {
                address: address.clone(),
                problem: format!("{}: {error}", path.display()),
            })?;

        // Read back rather than echo: a provider that repeats its input cannot
        // notice that it wrote nothing.
        self.entry_for(address, &path)?
            .map(|written| as_attributes(&path, &written))
            .ok_or_else(|| ProviderError::World {
                address: address.clone(),
                problem: format!("{} was not there after writing it", path.display()),
            })
    }

    fn delete(&self, address: &Address, recorded: &Attributes) -> Result<(), ProviderError> {
        let path = required_path(recorded, address)?;

        let removed = match &self.backend {
            Backend::Memory(entries) => {
                entries.borrow_mut().remove(&path);
                Ok(())
            }
            Backend::Disk => match self.entry_for(address, &path)? {
                // Already gone is the state that was asked for, not a failure.
                None => Ok(()),
                // ENFORCES: INV-DELETE-001
                // `remove_dir`, never `remove_dir_all`: a directory nerpa made
                // may since have been filled by somebody else, and recursively
                // deleting what it never created is not a removal, it is a
                // second, unannounced change. A non-empty directory fails here
                // and says so.
                Some(Entry::Directory { .. }) => fs::remove_dir(&path),
                Some(Entry::File { .. } | Entry::Symlink { .. }) => fs::remove_file(&path),
            },
        };

        removed.map_err(|error| ProviderError::World {
            address: address.clone(),
            problem: format!("{}: {error}", path.display()),
        })
    }

    fn fake() -> Self {
        #[allow(
            clippy::expect_used,
            reason = "a literal that stopped parsing is a bug here"
        )]
        Self {
            name: ProviderName::new("os").expect("`os` is a valid provider name"),
            backend: Backend::Memory(RefCell::new(BTreeMap::new())),
        }
    }
}
