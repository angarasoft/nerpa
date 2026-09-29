//! Files, directories and packages on a machine.
//!
//! The first provider. It owns `os:file`, `os:directory` and `os:package`, and
//! it does not touch a disk or run a program itself.
//!
//! Provider logic runs where the engine runs and the work happens where the
//! resource is (`0032`), so this is a **translator**:
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
mod service;

use package::Family;
use service::Init;

use std::collections::{BTreeMap, BTreeSet};

use nerpa_core::observed::Facts;
use nerpa_core::provider::{
    Afterwards, Answered, Attributes, Concluded, Concluding, Planned, Prepared, Provider,
    ProviderError, Reach, Read, Reading, Sending,
};
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
fn all(answer: Answer, address: &Address, asked: usize) -> Result<Vec<Seen>, ProviderError> {
    match answer {
        // Counted, because every caller reads these by position. One answer too
        // few and the last resource is described by nothing; one too many and each
        // is described by its neighbour's. Review found nothing checked.
        Answer::Found { seen } | Answer::Done { became: seen } if seen.len() == asked => Ok(seen),
        Answer::Found { seen } | Answer::Done { became: seen } => Err(ProviderError::World {
            address: address.clone(),
            problem: format!(
                "the far end answered {} things about {asked} it was asked, and they are read by position",
                seen.len()
            ),
        }),
        Answer::Moved { at } => Err(ProviderError::World {
            address: address.clone(),
            problem: format!("the machine is not what the plan was computed from, at premise {at}"),
        }),
        Answer::Failed { at, problem } => Err(ProviderError::World {
            address: address.clone(),
            problem: format!("{at}: {problem}"),
        }),
        other => Err(ProviderError::World {
            address: address.clone(),
            problem: format!("the far end said something unexpected: {other:?}"),
        }),
    }
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
    Service {
        address: Address,
        name: String,
        init: Init,
    },
}

impl Asked {
    fn address(&self) -> &Address {
        match self {
            Self::Path { address, .. }
            | Self::Package { address, .. }
            | Self::Service { address, .. } => address,
        }
    }

    /// What to look at. A list, because `OpenRC` keeps "running" and "starts at
    /// boot" in two places and there is no call that answers both.
    fn observations(&self) -> Vec<Observation> {
        match self {
            Self::Path { path, .. } => vec![Observation::Path { path: path.clone() }],
            Self::Package { atom, family, .. } => vec![family.query(atom)],
            Self::Service { name, init, .. } => init.look(name),
        }
    }

    fn understood(&self, seen: &[Seen]) -> Result<Option<Attributes>, ProviderError> {
        let first = || {
            seen.first().ok_or_else(|| ProviderError::World {
                address: self.address().clone(),
                problem: "the far end answered about nothing at all".to_owned(),
            })
        };
        match self {
            Self::Path { address, path } => as_attributes(path, first()?, address),
            Self::Package {
                address,
                atom,
                family,
            } => {
                let installed =
                    family
                        .installed(atom, first()?)
                        .map_err(|problem| ProviderError::World {
                            address: address.clone(),
                            problem,
                        })?;
                Ok(installed.map(|version| package::as_attributes(&version).into_iter().collect()))
            }
            Self::Service {
                address,
                name,
                init,
            } => {
                let found =
                    init.understood(name, seen)
                        .map_err(|problem| ProviderError::World {
                            address: address.clone(),
                            problem,
                        })?;
                Ok(found.map(|(running, enabled)| {
                    service::as_attributes(running, enabled)
                        .into_iter()
                        .collect()
                }))
            }
        }
    }
}

/// Which init system runs services on the machine this resource lives on.
///
/// Refused by name rather than guessed at, for the same reason a package family
/// is: a machine calling itself something this has not been taught gets an error
/// saying so, and nothing is sent on the chance that it works.
fn init(address: &Address, facts: Facts<'_>) -> Result<Init, ProviderError> {
    let Some(said) = facts.said("init") else {
        return Err(ProviderError::World {
            address: address.clone(),
            problem: "the machine did not say which init system it runs, so nothing \
                      knows how a service is started there"
                .to_owned(),
        });
    };
    Init::of(said).ok_or_else(|| ProviderError::World {
        address: address.clone(),
        problem: format!(
            "the machine runs {said:?}, and nerpa manages services under systemd \
             and openrc only"
        ),
    })
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
            "the machine says it is {said:?}, and nerpa manages packages on rhel-like, \
             debian-like and gentoo machines only"
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
#[dacc_derive::doc_anchor(id = "inv-plan-007-2")]
fn content_of(attributes: &Attributes, address: &Address) -> Result<Vec<u8>, ProviderError> {
    match attributes.get("content") {
        None => Ok(Vec::new()),
        Some(value) => {
            let content =
                value
                    .as_bytes()
                    .map(<[u8]>::to_vec)
                    .ok_or_else(|| ProviderError::Declaration {
                        address: address.clone(),
                        problem: format!("content holds {value}, which is neither text nor bytes"),
                    })?;
            // Planning refuses this already for every value it can see. Checked
            // again here for the one it cannot, a secret's, and because content
            // that overflows a frame fails in a way that names no resource.
            if content.len() > nerpa_proto::MAX_CONTENT {
                return Err(ProviderError::Declaration {
                    address: address.clone(),
                    problem: format!(
                        "content is {} bytes, and {} is the most one operation carries",
                        content.len(),
                        nerpa_proto::MAX_CONTENT
                    ),
                });
            }
            Ok(content)
        }
    }
}

/// Reads a mode written the way anybody writes one: `"0644"`, or `0o644`, which
/// Starlark makes a number.
fn mode(attributes: &Attributes, address: &Address, default: u32) -> Result<u32, ProviderError> {
    let Some(written) = attributes.get("mode") else {
        return Ok(default);
    };
    nerpa_core::schema::mode_of_value(written).ok_or_else(|| ProviderError::Declaration {
        address: address.clone(),
        problem: format!("mode {written} is not a file mode such as \"0644\""),
    })
}

fn required_path(attributes: &Attributes, address: &Address) -> Result<String, ProviderError> {
    text(attributes, "path").ok_or_else(|| ProviderError::Declaration {
        address: address.clone(),
        problem: "needs a `path`".to_owned(),
    })
}

/// What state recorded, which a removal and a replacement are built from.
fn recorded_of<'a>(change: &Prepared<'a>) -> Result<&'a Attributes, ProviderError> {
    change.recorded.ok_or_else(|| ProviderError::World {
        address: change.address.clone(),
        problem: "what is there now is described by what state recorded, and nothing was \
                  recorded"
            .to_owned(),
    })
}

/// The write that makes a path what the declaration says.
fn write_of(
    path: String,
    attributes: &Attributes,
    address: &Address,
    provider: &ProviderName,
    secret: &BTreeSet<String>,
) -> Result<Operation, ProviderError> {
    match address.kind().as_str() {
        "directory" => Ok(Operation::Directory {
            path,
            mode: mode(attributes, address, DEFAULT_DIRECTORY_MODE)?,
        }),
        // Marked by what it carries: the contents of a file are one of the two
        // places decision 0027 says a secret goes, and a write that carried one
        // without saying so would be answered with the value.
        "file" => Ok(Operation::File {
            path,
            content: content_of(attributes, address)?,
            mode: mode(attributes, address, DEFAULT_FILE_MODE)?,
            secret: carried(attributes)
                .iter()
                .any(|name| secret.contains(name.as_str())),
        }),
        _ => Err(ProviderError::UnknownKind {
            provider: provider.clone(),
            address: address.clone(),
        }),
    }
}

/// The declared attributes a write's answer confirms.
///
/// What the far end reads back after writing describes the path, its contents
/// and its mode, so a matching answer is a claim about exactly those — and only
/// about the ones the declaration named.
fn carried(attributes: &Attributes) -> BTreeSet<String> {
    ["content", "mode", "path"]
        .into_iter()
        .filter(|name| attributes.contains_key(*name))
        .map(ToOwned::to_owned)
        .collect()
}

/// A path is made, or removed — and replaced by doing both.
fn prepare_path(change: &Prepared<'_>, provider: &ProviderName) -> Result<Planned, ProviderError> {
    let address = change.address;
    if change.kind == Sending::Delete {
        return Ok(Planned {
            operations: vec![Operation::Remove {
                path: required_path(recorded_of(change)?, address)?,
            }],
            forget_after: None,
            carries: BTreeMap::new(),
        });
    }

    let mut operations = Vec::new();
    let mut forget_after = None;
    // Destroy before create, and only for a replacement: what is there is in the
    // way of what should be, and only state knows where the first one actually
    // is. The record is dropped between the two halves, because in between the
    // old one is gone and the new one is not there yet.
    if change.kind == Sending::Replace {
        operations.push(Operation::Remove {
            path: required_path(recorded_of(change)?, address)?,
        });
        forget_after = Some(operations.len());
    }

    let path = required_path(change.desired, address)?;
    operations.push(write_of(
        path,
        change.desired,
        address,
        provider,
        change.secret,
    )?);
    let mut carries = BTreeMap::new();
    carries.insert(operations.len() - 1, carried(change.desired));
    Ok(Planned {
        operations,
        forget_after,
        carries,
    })
}

/// What a path became, from what the far end read back after writing it.
/// What the far end saw, where that is what an answer carries.
fn saw<'a>(answered: &'a Answered, address: &Address) -> Result<&'a Seen, ProviderError> {
    match answered {
        Answered::Saw(seen) => Ok(seen),
        other => Err(ProviderError::World {
            address: address.clone(),
            problem: format!("this kind is concluded from what was seen, and was told {other:?}"),
        }),
    }
}

fn conclude_path(change: &Concluding<'_>, became: &Answered) -> Result<Concluded, ProviderError> {
    if change.kind == Sending::Delete {
        return Ok(Concluded::Gone);
    }
    let path = required_path(change.plain, change.address)?;
    match became {
        // The engine compared the write's digest against what it sent, so what
        // is at the path is what was sent. Nothing is read back, and a secret is
        // never handed to anything here to be read back with.
        Answered::Written => Ok(Concluded::Became(as_written(&path, change)?)),
        // A directory is still answered with what is there.
        other => as_attributes(&path, saw(other, change.address)?, change.address)?
            .map(Concluded::Became)
            .ok_or_else(|| ProviderError::World {
                address: change.address.clone(),
                problem: format!("{path} was not there after writing it"),
            }),
    }
}

/// What a confirmed write left at the path, in the shape a reading of it has.
///
/// Built from what was declared rather than from an answer, because the answer
/// is a digest the engine already checked. The shapes match what `read` reports
/// so that the next run has nothing to do: bytes for contents, four octal digits
/// for a mode.
fn as_written(path: &str, change: &Concluding<'_>) -> Result<Attributes, ProviderError> {
    let mut attributes = Attributes::new();
    attributes.insert("path".to_owned(), Value::Text(path.to_owned()));
    if change.plain.contains_key("content") {
        attributes.insert(
            "content".to_owned(),
            Value::bytes(content_of(change.plain, change.address)?),
        );
    }
    let mode = mode(change.plain, change.address, DEFAULT_FILE_MODE)?;
    attributes.insert("mode".to_owned(), Value::Text(format!("{mode:04o}")));
    Ok(attributes)
}

/// Two operations rather than one and a guess: the change, and then the same
/// question the read asked.
///
/// `dnf` and `emerge` both report success for work they did not do, so what the
/// machine ended up with is read back rather than echoed from the declaration.
fn prepare_package(change: &Prepared<'_>) -> Result<Planned, ProviderError> {
    let address = change.address;
    let atom = atom(address);
    let family = family(address, change.facts)?;

    // Installing a version already there runs no post-install, so sending a
    // package again is a different command — `dnf reinstall`, `emerge
    // --oneshot` — and nothing here has one yet. Refused by name rather than
    // sent on the chance that it works: a run that stopped between merging a
    // package and recording it leaves an entry, and the next plan sends that
    // change again (RFC 0001 §5.6). This is where it stops, with a sentence
    // saying what happened, rather than in a command that does nothing.
    if change.resent {
        return Err(ProviderError::World {
            address: address.clone(),
            problem: format!(
                "{atom} was left unrecorded by a run that stopped, and merging it a second \
                 time is a command this provider does not have yet"
            ),
        });
    }

    let operations = if change.kind == Sending::Delete {
        vec![family.remove(&atom), look_again(family, &atom)]
    } else {
        // Required, because "whatever is newest" is not a state a plan can
        // describe: what it does depends on when it runs. See 0038.
        let Some(version) = text(change.desired, "version") else {
            return Err(ProviderError::World {
                address: address.clone(),
                problem: "a package says which version it is, because \"the newest \
                          there is\" depends on when a run happens and no plan can \
                          carry a number over that"
                    .to_owned(),
            });
        };
        vec![family.install(&atom, &version), look_again(family, &atom)]
    };

    Ok(Planned {
        operations,
        forget_after: None,
        carries: BTreeMap::new(),
    })
}

/// What a package became, from the version the machine reports afterwards.
fn conclude_package(change: &Concluding<'_>, became: &Seen) -> Result<Concluded, ProviderError> {
    let address = change.address;
    let atom = atom(address);
    let family = family(address, change.facts)?;
    match (change.kind, family.installed(&atom, became)) {
        (Sending::Delete, Ok(None)) => Ok(Concluded::Gone),
        (Sending::Delete, Ok(Some(still))) => Err(ProviderError::World {
            address: address.clone(),
            problem: format!("{atom} is still installed, at {still}, after removing it"),
        }),
        (_, Ok(Some(version))) => Ok(Concluded::Became(
            package::as_attributes(&version).into_iter().collect(),
        )),
        (_, Ok(None)) => Err(ProviderError::World {
            address: address.clone(),
            problem: format!("{atom} is still not installed after installing it"),
        }),
        (_, Err(problem)) => Err(ProviderError::World {
            address: address.clone(),
            problem,
        }),
    }
}

/// Makes a service match what was declared, and reads back that it does.
///
/// A `React` change is a restart, not a start: being roused is what `reacts_to`
/// asks for, and a service that is already running would otherwise be left
/// exactly as it was by the change that exists to disturb it. See decision 0026.
///
/// A service that is not running is not restarted either — it was not running
/// before the file changed, and starting it now would be a change nobody
/// declared.
#[dacc_derive::doc_anchor(id = "inv-react-004-2")]
fn prepare_service(change: &Prepared<'_>) -> Result<Planned, ProviderError> {
    let address = change.address;
    let init = init(address, change.facts)?;
    let name = address.name().as_str();
    // `None` leaves the attribute alone: a service whose declaration does not
    // name `enabled` or `running` is not disabled or stopped for that omission,
    // and a value that is not a boolean is not this provider's to guess.
    let wanted = |what: &str| match change.desired.get(what) {
        Some(Value::Boolean(wanted)) => Some(*wanted),
        _ => None,
    };

    let mut operations = match change.kind {
        Sending::React => vec![init.rouse(name)],
        // Removing a declaration removes the declaration, not the unit: the unit
        // itself came from a package and stays, so nothing is stopped or disabled
        // (`w0010`). `conclude` says the resource is gone.
        Sending::Delete => Vec::new(),
        _ => {
            let mut settling = init.settle(name, wanted("running"), wanted("enabled"));
            // Roused as well as changed: settled first, then restarted if it is
            // running. This used to settle and stop there; `start` does nothing to a
            // running unit, and the fresh record cleared the debt, which left a new
            // file, an old process and nothing owed.
            if change.roused {
                settling.push(init.rouse(name));
            }
            settling
        }
    };
    // Whether it is running is read back rather than echoed: `systemctl start`
    // and `rc-service start` both report success for a service that came up and
    // fell over a second later, which is the ordinary way a service fails.
    operations.push(init.watch(name));

    Ok(Planned {
        operations,
        forget_after: None,
        carries: BTreeMap::new(),
    })
}

/// What a service became, from what it was doing after it was settled.
fn conclude_service(change: &Concluding<'_>, became: &Seen) -> Result<Concluded, ProviderError> {
    let address = change.address;
    let name = address.name().as_str();
    // A removal sent nothing to the unit and is concluded from the declaration
    // alone: the unit is left where it was, and as far as state is concerned the
    // resource is gone (`w0010`).
    if change.kind == Sending::Delete {
        return Ok(Concluded::Gone);
    }
    let init = init(address, change.facts)?;
    match init.running(name, became) {
        Ok(Some(running)) => {
            // `running` is read back. `enabled` is reported only when it was
            // declared: a service that never said `enabled` left it alone, and a
            // provider may not claim an attribute it was not given (`w0009`).
            let mut attributes = vec![("running".to_owned(), Value::Boolean(running))];
            if let Some(Value::Boolean(enabled)) = change.plain.get("enabled") {
                attributes.push(("enabled".to_owned(), Value::Boolean(*enabled)));
            }
            Ok(Concluded::Became(attributes.into_iter().collect()))
        }
        Ok(None) => Err(ProviderError::World {
            address: address.clone(),
            problem: format!("there is no service {name} on this machine after asking for it"),
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
            .bounded(
                self.name.clone(),
                kind("file"),
                "content",
                nerpa_proto::MAX_CONTENT,
            )
            // A file and a directory at one path are one thing on the machine.
            .identity(self.name.clone(), kind("file"), "path", "path")
            .identity(self.name.clone(), kind("directory"), "path", "path")
            // A mode is the number it spells, and the machine keeps only its
            // permission bits.
            .mode(self.name.clone(), kind("file"), "mode")
            .mode(self.name.clone(), kind("directory"), "mode")
            // A package has nothing a change elsewhere could ask it to do again:
            // on rhel installing it again did nothing, and on Portage it merged
            // again. So it cannot react, and a configuration saying it does is
            // refused where it says so.
            .cannot_react(self.name.clone(), kind("package"))
        // Nothing about a service is immutable: a stopped one starts, a
        // disabled one is enabled, and neither needs the resource replaced.
    }

    /// One journey for the whole batch, which is the only reason the batch is
    /// the unit. See `INV-BATCH-001`.
    #[dacc_derive::doc_anchor(id = "inv-premise-001-2")]
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
                "service" => about.push(Asked::Service {
                    address: address.clone(),
                    name: address.name().as_str().to_owned(),
                    init: init(address, facts)?,
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

        // Each entry says how many looks it needs, and gets exactly those back.
        let wanted: Vec<Vec<Observation>> = about.iter().map(Asked::observations).collect();
        let asked = wanted.iter().map(Vec::len).sum();
        let seen = all(
            reach.ask(&Ask::Look {
                observations: wanted.iter().flatten().cloned().collect(),
                seal: None,
            })?,
            &first,
            asked,
        )?;

        let mut answered = seen.into_iter();
        let mut found = Read::new();
        for (asked, mine) in about
            .into_iter()
            .zip(&wanted)
            .map(|(asked, wanted)| {
                let mine: Vec<Seen> = answered.by_ref().take(wanted.len()).collect();
                (asked, mine)
            })
            .collect::<Vec<_>>()
        {
            // What was looked at, and what it was when it was. The batch that
            // acts carries this back and the far end looks again before doing
            // anything — so a file that moved between the looking and the doing
            // stops the whole batch rather than being written over. Decision
            // 0034.
            let premise = asked
                .observations()
                .into_iter()
                .zip(&mine)
                .map(|(observation, answer)| Premise {
                    digest: nerpa_proto::digest(answer),
                    observation,
                })
                .collect();
            if let Some(attributes) = asked.understood(&mine)? {
                found = found.with(asked.address().clone(), attributes, premise);
            }
        }
        Ok(found)
    }

    fn prepare(&self, change: &Prepared<'_>) -> Result<Planned, ProviderError> {
        match change.address.kind().as_str() {
            "package" => prepare_package(change),
            "service" => prepare_service(change),
            "file" | "directory" => prepare_path(change, &self.name),
            _ => Err(ProviderError::UnknownKind {
                provider: self.name.clone(),
                address: change.address.clone(),
            }),
        }
    }

    fn conclude(
        &self,
        change: &Concluding<'_>,
        became: &[Answered],
    ) -> Result<Concluded, ProviderError> {
        // Every kind here reads its resource back with its last operation, so
        // that is the answer the change is concluded from.
        let Some(became) = became.last() else {
            return Err(ProviderError::World {
                address: change.address.clone(),
                problem: "the far end did not say what it became".to_owned(),
            });
        };
        match change.address.kind().as_str() {
            "package" => conclude_package(change, saw(became, change.address)?),
            "service" => conclude_service(change, saw(became, change.address)?),
            "file" | "directory" => conclude_path(change, became),
            _ => Err(ProviderError::UnknownKind {
                provider: self.name.clone(),
                address: change.address.clone(),
            }),
        }
    }

    /// A package is registered before its post-install has run, so finding it
    /// installed does not mean the change finished. Everything else this
    /// provider owns is seen in full by the next run's reading.
    fn afterwards(&self, address: &Address, _kind: Sending) -> Afterwards {
        if address.kind().as_str() == "package" {
            Afterwards::SendAgain
        } else {
            Afterwards::Read
        }
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
