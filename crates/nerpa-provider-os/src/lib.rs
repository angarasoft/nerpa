//! Files, directories and packages on a machine.
//!
//! The first provider. It owns `os:file`, `os:directory` and `os:package`, and
//! it does not touch a disk or run a program itself.
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
mod package;

use package::Family;

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
/// One thing this batch is looking at, and what its answer will mean.
///
/// A batch may hold both kinds at once — the engine groups by provider and
/// machine, not by kind — so each entry carries how to ask and how to read,
/// rather than the loop assuming every entry is the same shape.
#[derive(Debug)]
enum Asked {
    Path {
        address: Address,
        path: String,
    },
    Package {
        address: Address,
        atom: String,
        family: Family,
    },
}

impl Asked {
    fn address(&self) -> &Address {
        match self {
            Self::Path { address, .. } | Self::Package { address, .. } => address,
        }
    }

    fn observation(&self) -> Observation {
        match self {
            Self::Path { path, .. } => Observation::Path { path: path.clone() },
            Self::Package { atom, family, .. } => family.query(atom),
        }
    }

    fn understood(&self, seen: &Seen) -> Result<Option<Attributes>, ProviderError> {
        match self {
            Self::Path { address, path } => as_attributes(path, seen, address),
            Self::Package {
                address,
                atom,
                family,
            } => {
                let installed =
                    family
                        .installed(atom, seen)
                        .map_err(|problem| ProviderError::World {
                            address: address.clone(),
                            problem,
                        })?;
                Ok(installed.map(|version| package::as_attributes(&version).into_iter().collect()))
            }
        }
    }
}

/// The read's own question, asked again as an operation so that a batch can
/// both act and see what it did.
///
/// Its exit code is not checked: the answer is what it printed, and "not
/// installed" is a legitimate answer that `rpm` reports by exiting 1.
fn look_again(family: Family, atom: &str) -> Operation {
    let Observation::Output { program, arguments } = family.query(atom) else {
        unreachable!("a package is asked about by running a program")
    };
    Operation::Process {
        program,
        arguments,
        stdin: Vec::new(),
        private: false,
        expect: vec![0, 1],
    }
}

/// What a package is called where it lives.
///
/// The name and, where the declaration gave one, the slot — which in portage is
/// how `python:3.13` and `python:3.14` are two installed packages rather than a
/// conflict. See 0002 and 0038.
fn atom(address: &Address) -> String {
    match address.slot() {
        Some(slot) => format!("{}:{}", address.name().as_str(), slot.as_str()),
        None => address.name().as_str().to_owned(),
    }
}

/// Which family manages packages on the machine this resource lives on.
///
/// Refused by name rather than guessed at: a machine calling itself something
/// this provider has not been taught gets an error saying so, and no command is
/// sent on the chance that it works. See 0038.
fn family(address: &Address, facts: Facts<'_>) -> Result<Family, ProviderError> {
    let Some(said) = facts.said("os_family") else {
        return Err(ProviderError::World {
            address: address.clone(),
            problem: "the machine did not say what it is, so nothing knows how packages \
                      arrive there"
                .to_owned(),
        });
    };
    Family::of(said).ok_or_else(|| ProviderError::World {
        address: address.clone(),
        problem: format!(
            "the machine says it is {said:?}, and nerpa manages packages on rhel-like \
             and gentoo machines only"
        ),
    })
}

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

/// Makes a package present at the version the declaration names.
///
/// Two operations in one batch rather than one and a guess: the install, and
/// then the same question the read asked, so what the machine ended up with
/// is read back rather than echoed from the declaration. `dnf` and `emerge`
/// both report success for work they did not do, and a version that did not
/// change would otherwise be recorded as though it had.
fn install(
    address: &Address,
    attributes: &Attributes,
    facts: Facts<'_>,
    premise: &[Premise],
    reach: &dyn Reach,
) -> Result<Attributes, ProviderError> {
    let atom = atom(address);
    let family = family(address, facts)?;
    // Required, because "whatever is newest" is not a state a plan can
    // describe: what it does depends on when it runs. See 0038.
    let Some(version) = text(attributes, "version") else {
        return Err(ProviderError::World {
            address: address.clone(),
            problem: "a package says which version it is, because \"the newest \
                          there is\" depends on when a run happens and no plan can \
                          carry a number over that"
                .to_owned(),
        });
    };

    let became = all(
        reach.ask(&Ask::Perform {
            plan: None,
            premise: premise.to_vec(),
            operations: vec![family.install(&atom, &version), look_again(family, &atom)],
        })?,
        address,
    )?;
    let Some(afterwards) = became.last() else {
        return Err(ProviderError::World {
            address: address.clone(),
            problem: "the far end did not say what the package became".to_owned(),
        });
    };

    match family.installed(&atom, afterwards) {
        Ok(Some(became)) => Ok(package::as_attributes(&became).into_iter().collect()),
        Ok(None) => Err(ProviderError::World {
            address: address.clone(),
            problem: format!("{atom} is still not installed after installing it"),
        }),
        Err(problem) => Err(ProviderError::World {
            address: address.clone(),
            problem,
        }),
    }
}

/// Makes a package absent, and reads back that it is.
///
/// The same two operations as installing, for the same reason: a removal
/// that reported success and left the package there would be recorded as
/// gone.
fn uninstall(
    address: &Address,
    facts: Facts<'_>,
    premise: &[Premise],
    reach: &dyn Reach,
) -> Result<(), ProviderError> {
    let atom = atom(address);
    let family = family(address, facts)?;

    let became = all(
        reach.ask(&Ask::Perform {
            plan: None,
            premise: premise.to_vec(),
            operations: vec![family.remove(&atom), look_again(family, &atom)],
        })?,
        address,
    )?;
    let Some(afterwards) = became.last() else {
        return Err(ProviderError::World {
            address: address.clone(),
            problem: "the far end did not say what the package became".to_owned(),
        });
    };

    match family.installed(&atom, afterwards) {
        Ok(None) => Ok(()),
        Ok(Some(still)) => Err(ProviderError::World {
            address: address.clone(),
            problem: format!("{atom} is still installed, at {still}, after removing it"),
        }),
        Err(problem) => Err(ProviderError::World {
            address: address.clone(),
            problem,
        }),
    }
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
        facts: Facts<'_>,
        reach: &dyn Reach,
    ) -> Result<Read, ProviderError> {
        let mut about = Vec::new();

        for reading in batch {
            let address = reading.address();
            match address.kind().as_str() {
                "file" | "directory" => {
                    // No settled path means the declaration is waiting on
                    // something that has not happened. Nothing to look at, and
                    // nothing to guess.
                    let Some(path) = text(reading.locator(), "path") else {
                        continue;
                    };
                    about.push(Asked::Path {
                        address: address.clone(),
                        path,
                    });
                }
                "package" => about.push(Asked::Package {
                    address: address.clone(),
                    atom: atom(address),
                    family: family(address, facts)?,
                }),
                _ => {
                    return Err(ProviderError::UnknownKind {
                        provider: self.name.clone(),
                        address: address.clone(),
                    });
                }
            }
        }

        // Taken before the ask, so that a complaint about the answer has a real
        // resource to be about rather than one made up to fill the field.
        let Some(first) = about.first().map(|asked| asked.address().clone()) else {
            return Ok(Read::new());
        };

        let seen = all(
            reach.ask(&Ask::Look {
                observations: about.iter().map(Asked::observation).collect(),
            })?,
            &first,
        )?;

        let mut found = Read::new();
        for (asked, answer) in about.into_iter().zip(seen) {
            // ENFORCES: INV-PREMISE-001
            // What was looked at, and what it was when it was. The batch that
            // acts carries this back and the far end looks again before doing
            // anything — so a file that moved between the looking and the doing
            // stops the whole batch rather than being written over. Decision
            // 0034.
            let premise = vec![Premise {
                digest: nerpa_proto::digest(&answer),
                observation: asked.observation(),
            }];
            if let Some(attributes) = asked.understood(&answer)? {
                found = found.with(asked.address().clone(), attributes, premise);
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
        facts: Facts<'_>,
        reach: &dyn Reach,
    ) -> Result<Attributes, ProviderError> {
        if address.kind().as_str() == "package" {
            return install(address, attributes, facts, premise, reach);
        }

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
        facts: Facts<'_>,
        reach: &dyn Reach,
    ) -> Result<(), ProviderError> {
        if address.kind().as_str() == "package" {
            return uninstall(address, facts, premise, reach);
        }

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
