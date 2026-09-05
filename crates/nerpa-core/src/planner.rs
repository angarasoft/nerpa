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

use crate::attribute::Attribute;
use crate::graph::{Graph, GraphError, StageKind};
use crate::observed::Observed;
use crate::plan::{AttributeDelta, Change, ChangeKind, Confidence, Plan, PlanStage};
use crate::resource::Resource;
use crate::schema::Schema;
use crate::state::State;
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
    let mut stages = removals(graph, state);

    // One walk of the graph, in the order the work will happen. Stages are the
    // order of execution; there is no second pass over any of them.
    for stage in graph.stages()? {
        let mut changes = Vec::new();
        for address in stage.addresses() {
            let Some(resource) = graph.get(address) else {
                continue;
            };
            if let Some(change) = change_for(resource, observed, schema) {
                changes.push(change);
            }
        }
        // A stage where everything already matches is not a stage anybody needs
        // to read about.
        if !changes.is_empty() {
            stages.push(PlanStage::new(stage.kind().clone(), changes));
        }
    }

    Ok(Plan::new(stages))
}

/// What state remembers and the configuration no longer declares.
///
/// Removals come first, in reverse dependency order. First because a resource on
/// its way out can be holding something a new one needs — a name, a port, a path
/// — and because nothing being created can want something that is leaving.
fn removals(graph: &Graph, state: &State) -> Vec<PlanStage> {
    let declared: BTreeSet<&crate::address::Address> = state
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
fn change_for(resource: &Resource, observed: &Observed, schema: &Schema) -> Option<Change> {
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
