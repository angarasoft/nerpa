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
//!   Os ─── read  ─── what is on disk now, or nothing
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
use nerpa_core::provider::{Attributes, Provider, ProviderError};
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
    Directory { mode: u32 },
    File { content: String, mode: u32 },
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

    fn entry(&self, path: &Path) -> Option<Entry> {
        match &self.backend {
            Backend::Memory(entries) => entries.borrow().get(path).cloned(),
            Backend::Disk => read_from_disk(path),
        }
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

fn read_from_disk(path: &Path) -> Option<Entry> {
    let metadata = fs::symlink_metadata(path).ok()?;
    let mode = metadata.permissions().mode() & 0o7777;
    if metadata.is_dir() {
        return Some(Entry::Directory { mode });
    }
    let content = fs::read_to_string(path).ok()?;
    Some(Entry::File { content, mode })
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
            fs::write(path, content)?;
            fs::set_permissions(path, fs::Permissions::from_mode(*mode))
        }
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

    fn read(
        &self,
        address: &Address,
        declared: &Attributes,
    ) -> Result<Option<Attributes>, ProviderError> {
        match address.kind().as_str() {
            "file" | "directory" => {}
            _ => {
                return Err(ProviderError::UnknownKind {
                    provider: self.name.clone(),
                    address: address.clone(),
                });
            }
        }

        // No settled path means the declaration is waiting on something that has
        // not happened. Nothing to look at, and nothing to guess.
        let Some(path) = text(declared, "path").map(PathBuf::from) else {
            return Ok(None);
        };

        Ok(self.entry(&path).map(|entry| as_attributes(&path, &entry)))
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
        self.entry(&path)
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
            Backend::Disk => match self.entry(&path) {
                // Already gone is the state that was asked for, not a failure.
                None => Ok(()),
                Some(Entry::Directory { .. }) => fs::remove_dir_all(&path),
                Some(Entry::File { .. }) => fs::remove_file(&path),
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
