//! What one resource needs: the deltas, and the one change built from them.

use std::collections::{BTreeMap, BTreeSet};

use crate::address::Address;
use crate::attribute::Attribute;
use crate::observed::Observed;
use crate::plan::{AttributeDelta, Change, ChangeKind, Confidence, DeltaKind, Merge, Outdated};
use crate::resource::{Protection, Resource};
use crate::schema::Schema;
use crate::secret::{Recorded, Secrets};
use crate::state::Mark;
use crate::value::Value;

use super::PlanError;

/// What has to happen to one resource, or nothing if it already matches and is
/// recorded.
#[allow(
    clippy::too_many_arguments,
    reason = "what a change is decided from, one argument each, so none can be left out"
)]
/// One delta for every attribute that has to change, and the settled value of
/// every attribute the declaration names.
type Differences = (Vec<AttributeDelta>, Vec<(String, Confidence)>);

/// What the declaration asks for, and how each attribute differs from what is
/// there: the settled values a provider will be given, and one delta for every
/// name that has to change.
///
/// Every refusal that reads one attribute at a time lives here, so that a
/// resource is turned down while it is being described rather than after a kind
/// has been decided for it.
#[dacc_derive::doc_anchor(id = "planner-differences")]
pub(super) fn differences(
    resource: &Resource,
    existing: Option<&BTreeMap<String, Value>>,
    observed: &Observed,
    desired: &BTreeMap<Address, BTreeMap<String, Confidence>>,
    schema: &Schema,
    secrets: Secrets<'_>,
    outdated: &mut Vec<Outdated>,
) -> Result<Differences, PlanError> {
    let address = resource.address();
    let mut deltas = Vec::new();
    let mut wanted = Vec::new();
    for (name, declared) in resource.attributes() {
        let after = resolve(declared, observed, desired, secrets)?;
        validate_attribute(address, name, &after, schema)?;

        let mode = schema.is_mode(address, name);
        wanted.push((name.clone(), after.clone()));

        // Reduced here, at the edge: a secret attribute's observed value is a
        // real credential read back off a machine, and this is the last place it
        // is allowed to be one. Everything downstream sees a digest.
        let before = existing
            .and_then(|attributes| attributes.get(name))
            .map(|value| {
                if declared.is_secret() {
                    Recorded::Secret(secrets.digest(value))
                } else {
                    Recorded::Plain(value.clone())
                }
            });
        let matches = if mode {
            same_mode(before.as_ref(), &after)
        } else {
            settled(before.as_ref(), &after)
        };
        if matches {
            continue;
        }

        // Born with, not held to. A machine's boot disk cannot be swapped
        // underneath it, so this is reported and never turned into work — but
        // reported every time, because a fleet quietly years behind is the
        // failure this is meant to prevent. See decision 0021.
        if existing.is_some() && schema.is_at_birth(address, name) {
            outdated.push(Outdated::new(address.clone(), name.clone(), before, after));
            continue;
        }
        // A monotonic value may only grow. A declared value below what is
        // there is refused while planning rather than proposed as a change
        // the cloud would refuse anyway. Compared as integers; a shrink of a
        // value of another shape is the provider's to refuse.
        if existing.is_some()
            && schema.grows_only(address, name)
            && let Some(Recorded::Plain(Value::Integer(was))) = &before
            && let Confidence::Known(Value::Integer(will)) = &after
            && will < was
        {
            return Err(PlanError::ShrinkRefused {
                address: address.clone(),
                attribute: name.clone(),
                was: *was,
                will: *will,
            });
        }
        // A stop-to-change attribute is carried with the stop it requires,
        // and an immutable one still replaces: a replacement makes a stop
        // moot, so it wins where both are declared.
        let kind = if existing.is_some() && schema.forces_replacement(address, name) {
            DeltaKind::Replace
        } else if existing.is_some() && schema.requires_stop(address, name) {
            DeltaKind::StopThenChange
        } else {
            DeltaKind::InPlace
        };
        deltas.push(AttributeDelta::new(name.clone(), before, after, kind));
    }
    Ok((deltas, wanted))
}

/// Refuses one attribute while the resource is being described, so nothing is
/// done on the strength of a plan whose batch cannot be written.
fn validate_attribute(
    address: &Address,
    name: &str,
    after: &Confidence,
    schema: &Schema,
) -> Result<(), PlanError> {
    // A secret is sealed before this point and is checked by its provider
    // before sending instead.
    if let Confidence::Known(value) = after
        && let Some(most) = schema.bound(address, name)
        && let Some(bytes) = value.as_bytes()
        && bytes.len() > most
    {
        return Err(PlanError::TooLarge {
            address: address.clone(),
            attribute: name.to_owned(),
            size: bytes.len(),
            most,
        });
    }
    // A mode is the number the text it is written in spells. Compared as
    // written, "644" never matched
    // the "0644" a reading reports, and every run changed it again; one beyond
    // the permission bits never matched at all, because a machine keeps only
    // those. So it is compared as a number, and what spells no mode a file can
    // have is refused here, before anything is done on the strength of it.
    if schema.is_mode(address, name)
        && let Confidence::Known(written) = after
        && crate::schema::mode_of_value(written).is_none()
    {
        return Err(PlanError::NotAMode {
            address: address.clone(),
            attribute: name.to_owned(),
            value: match written {
                Value::Text(text) => text.clone(),
                other => other.to_string(),
            },
        });
    }
    Ok(())
}

/// What is known about one address besides its declaration and what was read
/// there: what this plan is doing around it, and what state remembers of it.
pub(super) struct Bearings<'a> {
    /// What this plan changes that it reacts to.
    pub(super) roused_by: Vec<Address>,
    /// What roused it in an earlier run that did not finish paying.
    pub(super) owed: Vec<Address>,
    /// Everything it comes after now, however that ordering arose.
    pub(super) depends_on: Vec<Address>,
    /// What state recorded it as coming after, if it records it at all.
    pub(super) recorded: Option<&'a BTreeSet<Address>>,
    /// The entry a run that stopped left under this address.
    pub(super) entry: Option<&'a Mark>,
    /// Whether a removal in this plan takes away what it would be read at, in
    /// which case nothing was read for it: see `taken_away`.
    pub(super) takes_over: bool,
}

#[dacc_derive::doc_anchor(id = "planner-change-for")]
#[allow(
    clippy::too_many_arguments,
    reason = "one pass over the graph; what one resource needs to decide is one argument each"
)]
pub(super) fn change_for(
    resource: &Resource,
    observed: &Observed,
    desired: &BTreeMap<Address, BTreeMap<String, Confidence>>,
    schema: &Schema,
    secrets: Secrets<'_>,
    outdated: &mut Vec<Outdated>,
    merges: &mut Vec<Merge>,
    around: Bearings<'_>,
) -> Result<Option<Change>, PlanError> {
    let Bearings {
        roused_by,
        owed,
        depends_on,
        recorded,
        entry,
        takes_over,
    } = around;
    let address = resource.address();
    // Not read when it names what a removal in this plan takes away: see
    // `taken_away`.
    let existing = if takes_over {
        None
    } else {
        observed.get(address)
    };

    let (deltas, wanted) = differences(
        resource, existing, observed, desired, schema, secrets, outdated,
    )?;

    // What an entry would send again. Worked out before the kind, and used only
    // where the declaration's own change would send nothing: a declaration that
    // still has work of its own is the better answer to what the machine is now.
    let resending = entry.and_then(Mark::sends_again);

    // Drift on a resource marked for merging is never corrected in place: it is
    // handed to a person as a three-way merge and reported here rather than
    // planned. INV-DRIFT-001.
    if resource.protection() == Protection::Merge && !deltas.is_empty() {
        merges.push(Merge::new(address.clone()));
        return Ok(None);
    }

    // The kind first, in the order it is decided — whether anything differs, then
    // why a resource that matches might still be work — and then the one change.
    let (kind, resent) = match (existing, resending) {
        // Not there at all: it has to be made, whatever its attributes say.
        (None, _) => (ChangeKind::Create, false),
        (Some(_), _)
            if deltas
                .iter()
                .any(|delta| delta.kind() == DeltaKind::Replace) =>
        {
            (ChangeKind::Replace, false)
        }
        (Some(_), _) if !deltas.is_empty() => (ChangeKind::Update, false),
        // It matches its declaration, and either something it reacts to does not
        // or an earlier run left a reaction unpaid. The desired values go with it
        // because the provider still needs to be told which service this is.
        (Some(_), _) if !(roused_by.is_empty() && owed.is_empty()) => (ChangeKind::React, false),
        // An earlier run marked this change and never recorded it, and what is
        // there now is what the declaration asks for. That is not proof the
        // change took: the reading would look the same if the run had stopped
        // between the operation and its answer, and the provider says reading
        // cannot tell the two apart. So it goes again rather than being adopted
        // or passed over, and it says so. RFC 0001 §5.6.
        (Some(_), Some(again)) => (again, true),
        // It matches its declaration and nothing records it: found on a machine
        // somebody configured before, or made by a run that stopped before it
        // could write that down. Nothing is done to it, and it is written down,
        // because from here on it is Nerpa's to remove. Planned as nothing, it
        // was never recorded, and taking its declaration away removed nothing.
        (Some(_), None) if recorded.is_none() => (ChangeKind::Adopt, false),
        // It matches and is recorded, and what it comes after is not what state
        // remembers: an `after` written in, a reference added, a reaction. Nothing
        // is done to it, and it is recorded again, because those edges are what
        // its removal will be ordered by. Planned as nothing, a file given an
        // `after` once it matched was removed with its directory in any order.
        (Some(_), None) if recorded.is_some_and(|edges| !edges.iter().eq(&depends_on)) => {
            (ChangeKind::Reorder, false)
        }
        (Some(_), None) => return Ok(None),
    };

    // What was read, sealed, for a change that sends something to a machine that
    // was looked at. A creation had nothing there to look at, a removal is decided
    // from state, and an adoption or a reorder sends nothing for a machine that
    // moved since to be made wrong by: the next reading corrects the record.
    // Every kind is named, so a new one has to say which it is.
    let premise = match kind {
        ChangeKind::Update | ChangeKind::Replace | ChangeKind::React => {
            sealed_for_its_machine(observed.premise(address), address, secrets)
        }
        ChangeKind::Create | ChangeKind::Delete | ChangeKind::Adopt | ChangeKind::Reorder => {
            Vec::new()
        }
    };

    let change = Change::new(
        address.clone(),
        kind,
        deltas,
        wanted,
        premise,
        roused_by,
        owed,
        depends_on,
    );
    Ok(Some(if resent {
        change.again(entry.is_some_and(Mark::is_past_forgetting))
    } else {
        change
    }))
}

/// What a change was computed from, sealed for the machine it was read on.
///
/// A premise is a digest of what is there now, and what is there now is
/// sometimes a secret. Unsealed, a plan's JSON carried an unkeyed digest of the
/// credential on disk, and a dictionary defeats an unkeyed digest of a short one
/// in seconds: found by a test, before any secret store was real. Sealed under a
/// key derived for that machine, it can be checked by the machine and tested by
/// nobody holding the plan without the state. See decision 0041.
fn sealed_for_its_machine(
    premise: &[nerpa_proto::Premise],
    address: &Address,
    secrets: Secrets<'_>,
) -> Vec<nerpa_proto::Premise> {
    // Not on a machine, so nothing read it there and nothing there will check it.
    let Some(node) = address.target() else {
        return Vec::new();
    };
    let seal = secrets.key().seal_for(node);
    premise
        .iter()
        .map(|held| nerpa_proto::Premise {
            observation: held.observation.clone(),
            digest: nerpa_proto::sealed(&seal, &held.digest),
        })
        .collect()
}

#[dacc_derive::doc_anchor(id = "planner-resolve")]
/// A declared attribute, with references resolved against what already exists.
pub(super) fn resolve(
    declared: &Attribute,
    observed: &Observed,
    desired: &BTreeMap<Address, BTreeMap<String, Confidence>>,
    secrets: Secrets<'_>,
) -> Result<Confidence, PlanError> {
    Ok(match declared {
        Attribute::Known(value) => Confidence::Known(value.clone()),
        Attribute::Unresolved(reference) => {
            match observed.attribute(reference.source(), reference.attribute()) {
                // The source exists. If it is changing in this same plan, read
                // the new value rather than the old one; otherwise the old value
                // is also the new one, and reading it is the same thing.
                Some(old) => match desired
                    .get(reference.source())
                    .and_then(|attributes| attributes.get(reference.attribute()))
                {
                    Some(Confidence::Known(new)) => Confidence::Known(new.clone()),
                    _ => Confidence::Known(old.clone()),
                },
                // The source does not exist yet: its value is settled once it is
                // made, and is not known while planning.
                None => Confidence::Unknown(reference.clone()),
            }
        }
        // Fetched to be hashed, and dropped in `seal`. The only way to say
        // whether a secret changed is to look at what is there now.
        Attribute::Secret(source) => Confidence::Secret(secrets.seal(source)?),
        // Every branch is written down, so the graph is known whichever way this
        // goes. What is open is the value, and only within the set — which the
        // plan can print, and a person can approve.
        Attribute::Chosen(choice) => {
            match observed.fact(choice.fact().node(), choice.fact().name()) {
                Some(said) => match choice.given(said) {
                    Some(value) => Confidence::Known(value.clone()),
                    // Caught here rather than at apply, which decision 0029
                    // expected: the fact is already known, so the plan can say
                    // what went wrong instead of a machine finding out later.
                    None => {
                        return Err(PlanError::Unaccounted {
                            fact: choice.fact().clone(),
                            said: said.to_owned(),
                        });
                    }
                },
                None => Confidence::Bounded(choice.clone()),
            }
        }
    })
}

/// Whether an attribute already holds what it should.
///
/// An unresolved value never matches: nothing can be said about a value that
/// does not exist, and claiming it matches would silently skip the write.
/// Whether a mode on the machine is the mode declared, as numbers.
fn same_mode(before: Option<&Recorded>, after: &Confidence) -> bool {
    match (before, after) {
        (Some(Recorded::Plain(current)), Confidence::Known(wanted)) => {
            crate::schema::mode_of_value(current).is_some()
                && crate::schema::mode_of_value(current) == crate::schema::mode_of_value(wanted)
        }
        _ => settled(before, after),
    }
}

fn settled(before: Option<&Recorded>, after: &Confidence) -> bool {
    match (before, after) {
        (Some(Recorded::Plain(current)), Confidence::Known(wanted)) => current == wanted,
        // Two digests under one key: equal means the credential on the machine
        // is the one the store holds, which is the whole question.
        (Some(Recorded::Secret(current)), Confidence::Secret(wanted)) => {
            *current == wanted.digest()
        }
        _ => false,
    }
}
