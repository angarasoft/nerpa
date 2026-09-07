//! Files and directories on a machine.
//!
//! The first provider, and deliberately the smallest useful one. It owns two
//! kinds — `os:file` and `os:directory` — and it does not touch a disk.
//!
//! Provider logic runs where the engine runs and the work happens where the
//! resource is ([0032](../../../docs/decisions)), so this is a **translator**:
//! it turns readings into things to look at, turns a change into things to do,
//! and turns the answers back into attributes. What actually opens a file is the
//! executor at the other end of a [`Reach`], and that is true whether the far
//! end is a pipe on this machine or `ssh` to another one.
//!
//! ```text
//!   Reading ──▶ Observation ──┐                 ┌──▶ Seen ──▶ Attributes
//!                             │  one journey    │
//!   Change  ──▶ Operation ────┴─── Reach ───────┘
//! ```
//!
//! # Correctness
//!
//! Everything is read back rather than echoed: `apply` reports what the resource
//! *became*, from what the far end saw after writing it. A provider that repeats
//! its input cannot notice that it wrote nothing, and the next plan would be
//! empty for the wrong reason.
//!
//! Unix only, and not apologetically: file modes are the whole point of the
//! `mode` attribute, and a portable abstraction over them would describe
//! something nobody runs.

use nerpa_core::address::{Address, Kind, Provider as ProviderName};
use nerpa_core::observed::Facts;
use nerpa_core::plan::ChangeKind;
use nerpa_core::provider::{Attributes, Provider, ProviderError, Reach, Read, Reading};
use nerpa_core::schema::Schema;
use nerpa_core::value::Value;
use nerpa_proto::{Answer, Ask, Entry, Observation, Operation, Premise, Seen};

/// Files and directories.
#[derive(Debug)]
pub struct Os {
    name: ProviderName,
}

impl Default for Os {
    fn default() -> Self {
        Self::new()
    }
}

const DEFAULT_FILE_MODE: u32 = 0o644;
const DEFAULT_DIRECTORY_MODE: u32 = 0o755;

impl Os {
    /// A provider that translates for the `os` kinds.
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
        }
    }
}

/// What the far end saw, or a refusal that carries its reason rather than
/// replacing it.
///
/// The first version answered "the far end did not answer a look with what it
/// found" for every shape it did not expect, which threw away the one thing
/// worth having — that a file could not be read, and why. A translator losing
/// the reason is the silent wrongness this project is about, committed by the
/// part meant to prevent it.
fn all(answer: Answer, address: &Address) -> Result<Vec<Seen>, ProviderError> {
    match answer {
        Answer::Found { seen } | Answer::Done { became: seen } => Ok(seen),
        Answer::Moved { at } => Err(ProviderError::World {
            address: address.clone(),
            problem: format!("the machine is not what the plan was computed from, at premise {at}"),
        }),
        Answer::Failed { at, problem } => Err(ProviderError::World {
            address: address.clone(),
            problem: format!("at {at}: {problem}"),
        }),
        other => Err(ProviderError::World {
            address: address.clone(),
            problem: format!("the far end said something unexpected: {other:?}"),
        }),
    }
}

/// The same, for an ask about exactly one thing.
fn one(answer: Answer, address: &Address) -> Result<Seen, ProviderError> {
    all(answer, address)?
        .into_iter()
        .next()
        .ok_or_else(|| ProviderError::World {
            address: address.clone(),
            problem: "the far end answered about nothing at all".to_owned(),
        })
}

/// What a reading of a path amounts to, as attributes.
///
/// `Ok(None)` is "there is nothing at that path", which is an answer. An error
/// is "the far end answered about something this never asked about", which is
/// the two ends having lost step — a different thing, and not one to report as
/// an absent resource.
fn as_attributes(
    path: &str,
    seen: &Seen,
    address: &Address,
) -> Result<Option<Attributes>, ProviderError> {
    let mut attributes = Attributes::new();
    attributes.insert("path".to_owned(), Value::Text(path.to_owned()));
    match seen {
        Seen::Nothing => return Ok(None),
        Seen::Entry(Entry::Directory { mode }) => {
            attributes.insert("mode".to_owned(), Value::Text(format!("{mode:04o}")));
        }
        Seen::Entry(Entry::File { content, mode }) => {
            attributes.insert("content".to_owned(), Value::bytes(content.clone()));
            attributes.insert("mode".to_owned(), Value::Text(format!("{mode:04o}")));
        }
        Seen::Entry(Entry::Symlink { target }) => {
            attributes.insert("symlink_to".to_owned(), Value::Text(target.clone()));
        }
        // This provider only ever asks about paths, so anything else means the
        // two ends disagree about what was asked.
        other => {
            return Err(ProviderError::World {
                address: address.clone(),
                problem: format!("asked about {path}, and was answered {other:?}"),
            });
        }
    }
    Ok(Some(attributes))
}

fn text(attributes: &Attributes, name: &str) -> Option<String> {
    match attributes.get(name)? {
        Value::Text(text) => Some(text.clone()),
        _ => None,
    }
}

/// What a file is to hold, from text or from bytes.
fn content_of(attributes: &Attributes, address: &Address) -> Result<Vec<u8>, ProviderError> {
    match attributes.get("content") {
        None => Ok(Vec::new()),
        Some(value) => {
            value
                .as_bytes()
                .map(<[u8]>::to_vec)
                .ok_or_else(|| ProviderError::Declaration {
                    address: address.clone(),
                    problem: format!("content holds {value}, which is neither text nor bytes"),
                })
        }
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

fn required_path(attributes: &Attributes, address: &Address) -> Result<String, ProviderError> {
    text(attributes, "path").ok_or_else(|| ProviderError::Declaration {
        address: address.clone(),
        problem: "needs a `path`".to_owned(),
    })
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

    /// One journey for the whole batch, which is the only reason the batch is
    /// the unit. See `INV-BATCH-001`.
    fn read(
        &self,
        batch: &[Reading],
        _facts: &Facts<'_>,
        reach: &dyn Reach,
    ) -> Result<Read, ProviderError> {
        let mut asking = Vec::new();
        let mut about = Vec::new();

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
            let Some(path) = text(reading.locator(), "path") else {
                continue;
            };
            asking.push(Observation::Path { path: path.clone() });
            about.push((address.clone(), path));
        }

        // Taken before the ask, so that a complaint about the answer has a real
        // resource to be about rather than one made up to fill the field.
        let Some((first, _)) = about.first().cloned() else {
            return Ok(Read::new());
        };

        let seen = all(
            reach.ask(&Ask::Look {
                observations: asking,
            })?,
            &first,
        )?;

        let mut found = Read::new();
        for ((address, path), answer) in about.into_iter().zip(seen) {
            // ENFORCES: INV-PREMISE-001
            // What was looked at, and what it was when it was. The batch that
            // acts carries this back and the far end looks again before doing
            // anything — so a file that moved between the looking and the doing
            // stops the whole batch rather than being written over. Decision
            // 0034.
            let premise = vec![Premise {
                digest: nerpa_proto::digest(&answer),
                observation: Observation::Path { path: path.clone() },
            }];
            if let Some(attributes) = as_attributes(&path, &answer, &address)? {
                found = found.with(address, attributes, premise);
            }
        }
        Ok(found)
    }

    fn apply(
        &self,
        address: &Address,
        _kind: ChangeKind,
        attributes: &Attributes,
        premise: &[Premise],
        reach: &dyn Reach,
    ) -> Result<Attributes, ProviderError> {
        let path = required_path(attributes, address)?;
        let operation = match address.kind().as_str() {
            "directory" => Operation::Directory {
                path: path.clone(),
                mode: mode(attributes, address, DEFAULT_DIRECTORY_MODE)?,
            },
            "file" => Operation::File {
                path: path.clone(),
                content: content_of(attributes, address)?,
                mode: mode(attributes, address, DEFAULT_FILE_MODE)?,
            },
            _ => {
                return Err(ProviderError::UnknownKind {
                    provider: self.name.clone(),
                    address: address.clone(),
                });
            }
        };

        let answer = reach.ask(&Ask::Perform {
            plan: None,
            premise: premise.to_vec(),
            operations: vec![operation],
        })?;

        // Read back rather than echoed: the far end reports what the path became
        // after it wrote, so writing nothing is noticed.
        let seen = one(answer, address)?;
        as_attributes(&path, &seen, address)?.ok_or_else(|| ProviderError::World {
            address: address.clone(),
            problem: format!("{path} was not there after writing it"),
        })
    }

    fn delete(
        &self,
        address: &Address,
        recorded: &Attributes,
        premise: &[Premise],
        reach: &dyn Reach,
    ) -> Result<(), ProviderError> {
        let path = required_path(recorded, address)?;
        let answer = reach.ask(&Ask::Perform {
            plan: None,
            premise: premise.to_vec(),
            operations: vec![Operation::Remove { path }],
        })?;
        one(answer, address).map(|_| ())
    }

    /// Nothing to fake.
    ///
    /// This provider decides things and touches nothing, so there is no world in
    /// it to stand in for. What a test wants instead is a [`Reach`] that answers
    /// without a machine — the fakery moved to the boundary that has one.
    fn fake() -> Self {
        Self::new()
    }
}
