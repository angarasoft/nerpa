//! Turning desired and observed state into a plan, in one pass.
//!
//! # Why one pass is enough
//!
//! The whole graph is known before anything runs, and so is everything that
//! could be read ([`Observed`]). What is genuinely not known are values that
//! will only exist once something has been created — and those are marked, not
//! waited for. Nothing here loops back to think again after an action.
//!
//! # Resolving a reference
//!
//! An attribute reading `db.ip` is unresolved in the declaration, but that is a
//! property of the text, not of the world. If `db` already exists and has an
//! `ip`, the value is known now and the attribute is compared like any other.
//! Without this a resource that reads from another would differ on every run and
//! never converge — the second run would never be empty, which is the one thing
//! it has to be.

use std::collections::BTreeSet;

use crate::address::{Address, Target};
use crate::attribute::Attribute;
use crate::graph::{Graph, GraphError, StageKind};
use crate::observed::Observed;
use crate::plan::{
    AttributeDelta, Change, ChangeKind, Confidence, Outdated, Plan, PlanStage, Unreachable,
};
use crate::resource::Resource;
use crate::schema::Schema;
use crate::state::{Record, State, Written};
use crate::value::Value;

/// Computes what would have to happen for the world to match the configuration.
///
/// # Errors
///
/// Returns [`GraphError`] if the configuration cannot be ordered at all — a
/// cycle, a dangling dependency, or a resource on a node the inventory does not
/// know. Nothing is planned for a graph that cannot be walked.
// ENFORCES: INV-PLAN-002
#[allow(
    clippy::result_large_err,
    reason = "diagnostics beat a lint on a cold path"
)]
pub fn plan(
    graph: &Graph,
    observed: &Observed,
    state: &State,
    schema: &Schema,
) -> Result<Plan, GraphError> {
    plan_at(graph, observed, state, schema, Written::now())
}

/// The same, with the clock supplied.
///
/// Only the age of stale knowledge needs a clock, and a test that asserts on an
/// age cannot have one moving underneath it.
///
/// # Errors
///
/// The same as [`plan`].
pub fn plan_at(
    graph: &Graph,
    observed: &Observed,
    state: &State,
    schema: &Schema,
    now: Written,
) -> Result<Plan, GraphError> {
    let mut stages = removals(graph, state);
    let mut unreachable = Vec::new();
    let mut outdated = Vec::new();
    // Marked as the walk goes. Stages are topological, so a resource's
    // dependencies have already been decided by the time it is reached, and
    // unreachability travels forward along the edges in a single pass.
    let mut unplannable: BTreeSet<Address> = BTreeSet::new();

    // One walk of the graph, in the order the work will happen. Stages are the
    // order of execution; there is no second pass over any of them.
    for stage in graph.stages()? {
        let mut changes = Vec::new();
        for address in stage.addresses() {
            let Some(resource) = graph.get(address) else {
                continue;
            };

            // ENFORCES: INV-REACH-001
            // Out of reach, or downstream of something that is. A resource whose
            // dependency could not be read has nowhere to take its values from,
            // so nothing can be said about it either.
            let node = address.domain().target().cloned();
            let out_of_reach = node.as_ref().is_some_and(|node| !observed.reached(node));
            let blocked = graph
                .dependencies_of(address)?
                .iter()
                .any(|dependency| unplannable.contains(dependency));

            if out_of_reach || blocked {
                unplannable.insert(address.clone());
                if let Some(node) = node {
                    let age = known_for(state, &node, now);
                    unreachable.push(Unreachable::new(address.clone(), node, age));
                }
                continue;
            }

            if let Some(change) = change_for(resource, observed, schema, &mut outdated) {
                changes.push(change);
            }
        }
        // A stage where everything already matches is not a stage anybody needs
        // to read about.
        if !changes.is_empty() {
            stages.push(PlanStage::new(stage.kind().clone(), changes));
        }
    }

    Ok(Plan::new(stages, unreachable, outdated))
}

/// How old the newest thing known about a machine is, in seconds.
///
/// Taken from state, the only record of when nerpa last saw it. `None` means the
/// machine has never been touched, so there is nothing stale to be old.
fn known_for(state: &State, node: &Target, now: Written) -> Option<u64> {
    state
        .addresses()
        .filter(|address| address.target() == Some(node))
        .filter_map(|address| state.get(address))
        .map(Record::written)
        .max()
        .map(|written| written.age(now).as_secs())
}

/// What state remembers and the configuration no longer declares.
///
/// Removals come first, in reverse dependency order. First because a resource on
/// its way out can be holding something a new one needs — a name, a port, a path
/// — and because nothing being created can want something that is leaving.
fn removals(graph: &Graph, state: &State) -> Vec<PlanStage> {
    let declared: BTreeSet<&Address> = state
        .addresses()
        .filter(|address| graph.get(address).is_some())
        .collect();

    state
        .abandoned(&declared)
        .into_iter()
        .map(|level| {
            let changes = level
                .into_iter()
                .map(|address| Change::new(address, ChangeKind::Delete, Vec::new(), Vec::new()))
                .collect();
            PlanStage::new(StageKind::Mixed, changes)
        })
        .collect()
}

/// What has to happen to one resource, or nothing if it already matches.
fn change_for(
    resource: &Resource,
    observed: &Observed,
    schema: &Schema,
    outdated: &mut Vec<Outdated>,
) -> Option<Change> {
    let address = resource.address();
    let existing = observed.get(address);

    let mut deltas = Vec::new();
    let mut desired = Vec::new();
    for (name, declared) in resource.attributes() {
        let after = resolve(declared, observed);
        desired.push((name.clone(), after.clone()));

        let before = existing
            .and_then(|attributes| attributes.get(name))
            .cloned();
        if settled(before.as_ref(), &after) {
            continue;
        }

        // Born with, not held to. A machine's boot disk cannot be swapped
        // underneath it, so this is reported and never turned into work — but
        // reported every time, because a fleet quietly years behind is the
        // failure this is meant to prevent. See decision 0021.
        // ENFORCES: INV-BIRTH-001
        if existing.is_some() && schema.is_at_birth(address, name) {
            outdated.push(Outdated::new(address.clone(), name.clone(), before, after));
            continue;
        }
        let forces_replacement = existing.is_some() && schema.forces_replacement(address, name);
        deltas.push(AttributeDelta::new(
            name.clone(),
            before,
            after,
            forces_replacement,
        ));
    }

    match existing {
        // Not there at all: it has to be made, whatever its attributes say.
        None => Some(Change::new(
            address.clone(),
            ChangeKind::Create,
            deltas,
            desired,
        )),
        Some(_) if deltas.is_empty() => None,
        Some(_) => {
            let kind = if deltas.iter().any(AttributeDelta::forces_replacement) {
                ChangeKind::Replace
            } else {
                ChangeKind::Update
            };
            Some(Change::new(address.clone(), kind, deltas, desired))
        }
    }
}

// ENFORCES: INV-PLAN-003
/// A declared attribute, with references resolved against what already exists.
fn resolve(declared: &Attribute, observed: &Observed) -> Confidence {
    match declared {
        Attribute::Known(value) => Confidence::Known(value.clone()),
        Attribute::Unresolved(reference) => {
            match observed.attribute(reference.source(), reference.attribute()) {
                Some(value) => Confidence::Known(value.clone()),
                None => Confidence::Unknown(reference.clone()),
            }
        }
    }
}

/// Whether an attribute already holds what it should.
///
/// An unresolved value never matches: nothing can be said about a value that
/// does not exist, and claiming it matches would silently skip the write.
fn settled(before: Option<&Value>, after: &Confidence) -> bool {
    match (before, after) {
        (Some(current), Confidence::Known(wanted)) => current == wanted,
        _ => false,
    }
}
