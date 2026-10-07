//! Conversions between the WIT bindings and the in-tree types.
//!
//! The mirror is symmetric: each type has a conversion in each direction. The
//! host uses one direction of each — it reads the component's output and never
//! composes a wire type itself — and the other is kept so the two stay honest
//! about the same shape.

#![allow(
    dead_code,
    reason = "each conversion has a reverse the host does not use; kept so the mirror is symmetric"
)]

use std::collections::BTreeMap;

use nerpa_core::address::{Address, Kind, Name, Provider, Slot, Target};
use nerpa_core::observed::Facts;
use nerpa_core::provider::{
    Afterwards, Answered, Attributes, Concluded, Concluding, Planned, Prepared, Read, Reading,
    Sending,
};
use nerpa_core::value::Value;
use nerpa_proto::{Answer, Ask, At, Entry, Observation, Operation, Premise, Seal, Seen};

use crate::bindings;

/// A settled value travels as CBOR bytes; both ends use `nerpa_core::Value`.
pub(crate) fn value_to_wit(value: &Value) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    ciborium::ser::into_writer(value, &mut bytes).map_err(|error| error.to_string())?;
    Ok(bytes)
}

pub(crate) fn value_from_wit(bytes: &[u8]) -> Result<Value, String> {
    ciborium::de::from_reader(bytes).map_err(|error| error.to_string())
}

pub(crate) fn attributes_to_wit(attributes: &Attributes) -> Result<Vec<(String, Vec<u8>)>, String> {
    attributes
        .iter()
        .map(|(name, value)| Ok((name.clone(), value_to_wit(value)?)))
        .collect()
}

pub(crate) fn attributes_from_wit(attributes: &[(String, Vec<u8>)]) -> Result<Attributes, String> {
    attributes
        .iter()
        .map(|(name, bytes)| Ok((name.clone(), value_from_wit(bytes)?)))
        .collect::<Result<BTreeMap<_, _>, String>>()
}

pub(crate) fn address_to_wit(address: &Address) -> bindings::Address {
    bindings::Address {
        provider: address.provider().as_str().to_owned(),
        kind: address.kind().as_str().to_owned(),
        name: address.name().as_str().to_owned(),
        slot: address.slot().map(|slot| slot.as_str().to_owned()),
        target: address.target().map(|target| target.as_str().to_owned()),
    }
}

pub(crate) fn address_from_wit(wit: &bindings::Address) -> Result<Address, String> {
    let mut address = Address::control(
        Provider::new(&wit.provider).map_err(|error| error.to_string())?,
        Kind::new(&wit.kind).map_err(|error| error.to_string())?,
        Name::new(&wit.name).map_err(|error| error.to_string())?,
    );
    if let Some(slot) = &wit.slot {
        address = address.in_slot(Slot::new(slot).map_err(|error| error.to_string())?);
    }
    if let Some(target) = &wit.target {
        address = address.on(Target::new(target).map_err(|error| error.to_string())?);
    }
    Ok(address)
}

pub(crate) fn facts_to_wit(facts: Facts<'_>) -> Vec<(String, String)> {
    facts
        .all()
        .iter()
        .map(|(name, said)| (name.clone(), said.clone()))
        .collect()
}

pub(crate) fn sending_to_wit(kind: Sending) -> bindings::Sending {
    match kind {
        Sending::Create => bindings::Sending::Create,
        Sending::Update => bindings::Sending::Update,
        Sending::Replace => bindings::Sending::Replace,
        Sending::Delete => bindings::Sending::Delete,
        Sending::React => bindings::Sending::React,
        _ => unreachable!("a sending kind this build does not know"),
    }
}

pub(crate) fn sending_from_wit(kind: bindings::Sending) -> Sending {
    match kind {
        bindings::Sending::Create => Sending::Create,
        bindings::Sending::Update => Sending::Update,
        bindings::Sending::Replace => Sending::Replace,
        bindings::Sending::Delete => Sending::Delete,
        bindings::Sending::React => Sending::React,
    }
}

pub(crate) fn afterwards_to_wit(kind: Afterwards) -> bindings::Afterwards {
    match kind {
        Afterwards::Read => bindings::Afterwards::Read,
        Afterwards::SendAgain => bindings::Afterwards::SendAgain,
    }
}

pub(crate) fn afterwards_from_wit(kind: bindings::Afterwards) -> Afterwards {
    match kind {
        bindings::Afterwards::Read => Afterwards::Read,
        bindings::Afterwards::SendAgain => Afterwards::SendAgain,
    }
}

pub(crate) fn reading_to_wit(reading: &Reading) -> Result<bindings::Reading, String> {
    Ok(bindings::Reading {
        address: address_to_wit(reading.address()),
        locator: attributes_to_wit(reading.locator())?,
    })
}

pub(crate) fn read_result_from_wit(read: &bindings::ReadResult) -> Result<Read, String> {
    let mut result = Read::new();
    for (address, attributes) in &read.attributes {
        result = result.with(
            address_from_wit(address)?,
            attributes_from_wit(attributes)?,
            Vec::new(),
        );
    }
    for (address, premise) in &read.premise {
        let premise = premise
            .iter()
            .map(premise_from_wit)
            .collect::<Result<Vec<_>, _>>()?;
        result = result.with(address_from_wit(address)?, BTreeMap::new(), premise);
    }
    Ok(result)
}

pub(crate) fn prepared_to_wit(change: &Prepared<'_>) -> Result<bindings::Prepared, String> {
    Ok(bindings::Prepared {
        address: address_to_wit(change.address),
        kind: sending_to_wit(change.kind),
        roused: change.roused,
        resent: change.resent,
        desired: attributes_to_wit(change.desired)?,
        secret: change.secret.iter().cloned().collect(),
        recorded: change.recorded.map(attributes_to_wit).transpose()?,
        facts: facts_to_wit(change.facts),
    })
}

pub(crate) fn planned_from_wit(planned: &bindings::Planned) -> Planned {
    Planned {
        operations: planned.operations.iter().map(operation_from_wit).collect(),
        forget_after: planned.forget_after.map(index_usize),
        carries: planned
            .carries
            .iter()
            .map(|(index, names)| (index_usize(*index), names.iter().cloned().collect()))
            .collect(),
    }
}

pub(crate) fn concluding_to_wit(change: &Concluding<'_>) -> Result<bindings::Concluding, String> {
    Ok(bindings::Concluding {
        address: address_to_wit(change.address),
        kind: sending_to_wit(change.kind),
        roused: change.roused,
        facts: facts_to_wit(change.facts),
        plain: attributes_to_wit(change.plain)?,
        secret: change.secret.iter().cloned().collect(),
    })
}

pub(crate) fn answered_to_wit(answered: &Answered) -> bindings::Answered {
    match answered {
        Answered::Saw(seen) => bindings::Answered::Saw(seen_to_wit(seen)),
        Answered::Written => bindings::Answered::Written,
        _ => unreachable!("an answer this build does not know"),
    }
}

pub(crate) fn concluded_from_wit(concluded: &bindings::Concluded) -> Result<Concluded, String> {
    match concluded {
        bindings::Concluded::Became(attributes) => {
            Ok(Concluded::Became(attributes_from_wit(attributes)?))
        }
        bindings::Concluded::Gone => Ok(Concluded::Gone),
    }
}

fn seal_to_wit(seal: Option<Seal>) -> Option<Vec<u8>> {
    seal.map(|seal| seal.0.to_vec())
}

fn seal_from_wit(seal: Option<&[u8]>) -> Result<Option<Seal>, String> {
    seal.map(|bytes| {
        let bytes: [u8; 32] = bytes
            .try_into()
            .map_err(|_| "a seal is 32 bytes".to_owned())?;
        Ok(Seal(bytes))
    })
    .transpose()
}

fn digest_from_wit(bytes: &[u8]) -> Result<[u8; 32], String> {
    bytes
        .try_into()
        .map_err(|_| "a digest is 32 bytes".to_owned())
}

fn observation_to_wit(observation: &Observation) -> bindings::Observation {
    match observation {
        Observation::Path { path } => bindings::Observation::Path(path.clone()),
        Observation::Contents { path } => bindings::Observation::Contents(path.clone()),
        Observation::Output { program, arguments } => {
            bindings::Observation::Output(bindings::Output {
                program: program.clone(),
                arguments: arguments.clone(),
            })
        }
        Observation::Sealed { of } => bindings::Observation::Sealed(sealed_of_to_wit(of)),
        _ => unreachable!("an observation this build does not know"),
    }
}

fn sealed_of_to_wit(of: &Observation) -> bindings::SealedOf {
    match of {
        Observation::Path { path } => bindings::SealedOf::Path(path.clone()),
        Observation::Contents { path } => bindings::SealedOf::Contents(path.clone()),
        Observation::Output { program, arguments } => {
            bindings::SealedOf::Output(bindings::Output {
                program: program.clone(),
                arguments: arguments.clone(),
            })
        }
        Observation::Sealed { .. } => unreachable!("one seal inside another is refused"),
        _ => unreachable!("an observation this build does not know"),
    }
}

fn observation_from_wit(observation: &bindings::Observation) -> Observation {
    match observation {
        bindings::Observation::Path(path) => Observation::Path { path: path.clone() },
        bindings::Observation::Contents(path) => Observation::Contents { path: path.clone() },
        bindings::Observation::Output(output) => Observation::Output {
            program: output.program.clone(),
            arguments: output.arguments.clone(),
        },
        bindings::Observation::Sealed(of) => Observation::Sealed {
            of: Box::new(sealed_of_from_wit(of)),
        },
    }
}

fn sealed_of_from_wit(of: &bindings::SealedOf) -> Observation {
    match of {
        bindings::SealedOf::Path(path) => Observation::Path { path: path.clone() },
        bindings::SealedOf::Contents(path) => Observation::Contents { path: path.clone() },
        bindings::SealedOf::Output(output) => Observation::Output {
            program: output.program.clone(),
            arguments: output.arguments.clone(),
        },
    }
}

fn seen_to_wit(seen: &Seen) -> bindings::Seen {
    match seen {
        Seen::Nothing => bindings::Seen::Nothing,
        Seen::Entry(entry) => bindings::Seen::Entry(entry_to_wit(entry)),
        Seen::Said {
            status,
            out,
            complaint,
        } => bindings::Seen::Said(bindings::Said {
            status: *status,
            out: out.clone(),
            complaint: complaint.clone(),
        }),
        Seen::Sealed { digest } => bindings::Seen::Sealed(digest.to_vec()),
        Seen::Private { status, sealed } => bindings::Seen::Private(bindings::Private {
            status: *status,
            sealed: sealed.to_vec(),
        }),
        Seen::Digest { digest } => bindings::Seen::Digest(digest.to_vec()),
        _ => unreachable!("a seen this build does not know"),
    }
}

fn seen_from_wit(seen: &bindings::Seen) -> Result<Seen, String> {
    Ok(match seen {
        bindings::Seen::Nothing => Seen::Nothing,
        bindings::Seen::Entry(entry) => Seen::Entry(entry_from_wit(entry)),
        bindings::Seen::Said(said) => Seen::Said {
            status: said.status,
            out: said.out.clone(),
            complaint: said.complaint.clone(),
        },
        bindings::Seen::Sealed(digest) => Seen::Sealed {
            digest: digest_from_wit(digest)?,
        },
        bindings::Seen::Private(private) => Seen::Private {
            status: private.status,
            sealed: digest_from_wit(&private.sealed)?,
        },
        bindings::Seen::Digest(digest) => Seen::Digest {
            digest: digest_from_wit(digest)?,
        },
    })
}

fn entry_to_wit(entry: &Entry) -> bindings::Entry {
    match entry {
        Entry::Directory { mode } => bindings::Entry::Directory(*mode),
        Entry::File { content, mode } => bindings::Entry::File(bindings::FileEntry {
            content: content.clone(),
            mode: *mode,
        }),
        Entry::Symlink { target } => bindings::Entry::Symlink(target.clone()),
        _ => unreachable!("an entry this build does not know"),
    }
}

fn entry_from_wit(entry: &bindings::Entry) -> Entry {
    match entry {
        bindings::Entry::Directory(mode) => Entry::Directory { mode: *mode },
        bindings::Entry::File(file) => Entry::File {
            content: file.content.clone(),
            mode: file.mode,
        },
        bindings::Entry::Symlink(target) => Entry::Symlink {
            target: target.clone(),
        },
    }
}

fn premise_to_wit(premise: &Premise) -> bindings::Premise {
    bindings::Premise {
        observation: observation_to_wit(&premise.observation),
        digest: premise.digest.to_vec(),
    }
}

fn premise_from_wit(premise: &bindings::Premise) -> Result<Premise, String> {
    Ok(Premise {
        observation: observation_from_wit(&premise.observation),
        digest: digest_from_wit(&premise.digest)?,
    })
}

fn operation_to_wit(operation: &Operation) -> bindings::Operation {
    match operation {
        Operation::Directory { path, mode } => {
            bindings::Operation::Directory(bindings::DirectoryOperation {
                path: path.clone(),
                mode: *mode,
            })
        }
        Operation::File {
            path,
            content,
            mode,
            secret,
        } => bindings::Operation::File(bindings::FileOperation {
            path: path.clone(),
            content: content.clone(),
            mode: *mode,
            secret: *secret,
        }),
        Operation::Process {
            program,
            arguments,
            stdin,
            private,
            expect,
        } => bindings::Operation::Process(bindings::ProcessOperation {
            program: program.clone(),
            arguments: arguments.clone(),
            stdin: stdin.clone(),
            private: *private,
            expect: expect.clone(),
        }),
        Operation::Remove { path } => bindings::Operation::Remove(path.clone()),
        _ => unreachable!("an operation this build does not know"),
    }
}

fn operation_from_wit(operation: &bindings::Operation) -> Operation {
    match operation {
        bindings::Operation::Directory(directory) => Operation::Directory {
            path: directory.path.clone(),
            mode: directory.mode,
        },
        bindings::Operation::File(file) => Operation::File {
            path: file.path.clone(),
            content: file.content.clone(),
            mode: file.mode,
            secret: file.secret,
        },
        bindings::Operation::Process(process) => Operation::Process {
            program: process.program.clone(),
            arguments: process.arguments.clone(),
            stdin: process.stdin.clone(),
            private: process.private,
            expect: process.expect.clone(),
        },
        bindings::Operation::Remove(path) => Operation::Remove { path: path.clone() },
    }
}

/// An index the wire counts in 32 bits; the engine counts in `usize`.
fn index_u32(index: usize) -> u32 {
    u32::try_from(index).expect("an index fits in 32 bits")
}

fn index_usize(index: u32) -> usize {
    usize::try_from(index).expect("a 32-bit index fits in usize")
}

fn ask_to_wit(ask: &Ask) -> bindings::Ask {
    match ask {
        Ask::Look { observations, seal } => bindings::Ask::Look(bindings::Look {
            observations: observations.iter().map(observation_to_wit).collect(),
            seal: seal_to_wit(*seal),
        }),
        Ask::Perform {
            plan,
            premise,
            operations,
            seal,
        } => bindings::Ask::Perform(bindings::Perform {
            plan: plan.map(|plan| plan.to_vec()),
            premise: premise.iter().map(premise_to_wit).collect(),
            operations: operations.iter().map(operation_to_wit).collect(),
            seal: seal_to_wit(*seal),
        }),
        Ask::Fingerprint => bindings::Ask::Fingerprint,
        Ask::Build => bindings::Ask::Build,
        _ => unreachable!("an ask this build does not know"),
    }
}

pub(crate) fn ask_from_wit(ask: &bindings::Ask) -> Result<Ask, String> {
    Ok(match ask {
        bindings::Ask::Look(look) => Ask::Look {
            observations: look.observations.iter().map(observation_from_wit).collect(),
            seal: seal_from_wit(look.seal.as_deref())?,
        },
        bindings::Ask::Perform(perform) => Ask::Perform {
            plan: perform
                .plan
                .as_ref()
                .map(|plan| digest_from_wit(plan))
                .transpose()?,
            premise: perform
                .premise
                .iter()
                .map(premise_from_wit)
                .collect::<Result<_, _>>()?,
            operations: perform.operations.iter().map(operation_from_wit).collect(),
            seal: seal_from_wit(perform.seal.as_deref())?,
        },
        bindings::Ask::Fingerprint => Ask::Fingerprint,
        bindings::Ask::Build => Ask::Build,
    })
}

pub(crate) fn answer_to_wit(answer: &Answer) -> bindings::Answer {
    match answer {
        Answer::Found { seen } => bindings::Answer::Found(seen.iter().map(seen_to_wit).collect()),
        Answer::Done { became } => bindings::Answer::Done(became.iter().map(seen_to_wit).collect()),
        Answer::Moved { at } => bindings::Answer::Moved(index_u32(*at)),
        Answer::Fingerprint { digest } => bindings::Answer::Fingerprint(digest.clone()),
        Answer::Build { digest } => bindings::Answer::Build(digest.clone()),
        Answer::Failed { at, problem } => bindings::Answer::Failed(bindings::Failed {
            at: at_to_wit(*at),
            problem: problem.clone(),
        }),
        _ => unreachable!("an answer this build does not know"),
    }
}

fn answer_from_wit(answer: &bindings::Answer) -> Result<Answer, String> {
    Ok(match answer {
        bindings::Answer::Found(seen) => Answer::Found {
            seen: seen.iter().map(seen_from_wit).collect::<Result<_, _>>()?,
        },
        bindings::Answer::Done(became) => Answer::Done {
            became: became.iter().map(seen_from_wit).collect::<Result<_, _>>()?,
        },
        bindings::Answer::Moved(at) => Answer::Moved {
            at: index_usize(*at),
        },
        bindings::Answer::Fingerprint(digest) => Answer::Fingerprint {
            digest: digest.clone(),
        },
        bindings::Answer::Build(digest) => Answer::Build {
            digest: digest.clone(),
        },
        bindings::Answer::Failed(failed) => Answer::Failed {
            at: at_from_wit(failed.at),
            problem: failed.problem.clone(),
        },
    })
}

fn at_to_wit(at: At) -> bindings::At {
    match at {
        At::Before => bindings::At::Before,
        At::Premise(index) => bindings::At::Premise(index_u32(index)),
        At::Operation(index) => bindings::At::Operation(index_u32(index)),
        At::Observation(index) => bindings::At::Observation(index_u32(index)),
        _ => unreachable!("an at this build does not know"),
    }
}

fn at_from_wit(at: bindings::At) -> At {
    match at {
        bindings::At::Before => At::Before,
        bindings::At::Premise(index) => At::Premise(index_usize(index)),
        bindings::At::Operation(index) => At::Operation(index_usize(index)),
        bindings::At::Observation(index) => At::Observation(index_usize(index)),
    }
}
