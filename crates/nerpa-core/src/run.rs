//! Reading the world, and making it match.
//!
//! Two walks, and they are not symmetrical. Reading happens once, before
//! anything else, so that the plan is complete before the first action
//! ([0006](../../../docs/decisions)). Applying walks the plan in the order the
//! plan already fixed, and resolves as it goes: a value that was unknown at plan
//! time is read out of what an earlier stage produced.
//!
//! Nothing here touches the world itself. It calls providers, which do.

use std::collections::BTreeSet;

use crate::address::Address;
use crate::attribute::Attribute;
use crate::graph::Graph;
use crate::observed::Observed;
use crate::plan::{Change, ChangeKind, Confidence, Plan};
use crate::provider::{Attributes, ProviderError, Providers};
use crate::resource::Resource;
use crate::state::{Record, State};
use crate::{ensures, requires};

/// Reads every resource the configuration declares.
///
/// # Errors
///
/// Returns [`ProviderError`] if any resource could not be read. A partial
/// reading is not a smaller reading: planning against it would propose
/// creating things that are already there.
pub fn observe(
    graph: &Graph,
    providers: &Providers,
    state: &State,
) -> Result<Observed, ProviderError> {
    let mut observed = Observed::new();

    for stage in graph.stages().into_iter().flatten() {
        for address in stage.addresses() {
            let Some(resource) = graph.get(address) else {
                continue;
            };
            // ENFORCES: INV-STATE-003
            // Where it is, not where the declaration now says it should be. A
            // resource whose location changed has to be found where it was left,
            // or the change reads as making a second one and the first is
            // abandoned.
            let locator = state.get(address).map_or_else(
                || settled_declaration(resource),
                |record| record.attributes().clone(),
            );

            let provider = providers.for_address(address)?;
            if let Some(attributes) = provider.read(address, &locator)? {
                observed.record(address.clone(), attributes);
            }
        }
    }

    // Anything state remembers and the configuration no longer declares still
    // has to be read, or its removal would be planned blind.
    for address in state.addresses() {
        if graph.get(address).is_some() {
            continue;
        }
        let Some(record) = state.get(address) else {
            continue;
        };
        let provider = providers.for_address(address)?;
        if let Some(attributes) = provider.read(address, record.attributes())? {
            observed.record(address.clone(), attributes);
        }
    }

    Ok(observed)
}

/// The declared attributes that are values already.
///
/// Anything still waiting on another resource is left out rather than guessed
/// at. A resource whose location depends on something not yet created is not
/// there, and reading somewhere else would be worse than reading nowhere.
fn settled_declaration(resource: &Resource) -> Attributes {
    resource
        .attributes()
        .iter()
        .filter_map(|(name, attribute)| match attribute {
            Attribute::Known(value) => Some((name.clone(), value.clone())),
            Attribute::Unresolved(_) => None,
        })
        .collect()
}

/// What a run actually did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Applied {
    observed: Observed,
    changed: usize,
}

impl Applied {
    /// What the world looks like now, as the providers reported it back.
    #[must_use]
    pub fn observed(&self) -> &Observed {
        &self.observed
    }

    /// How many resources were changed.
    #[must_use]
    pub fn changed(&self) -> usize {
        self.changed
    }
}

/// Carries out a plan, stage by stage.
///
/// # Errors
///
/// Returns [`ProviderError`] at the first change that fails. Earlier stages have
/// already happened and are reported in the [`Applied`] the error replaces —
/// nothing is rolled back, because there is nothing that could roll a package
/// installation back.
///
/// # Panics
///
/// Panics if a value is still unresolved when its stage runs, which would mean
/// the plan's own ordering was wrong. See [`crate::contract`].
pub fn apply(
    plan: &Plan,
    providers: &Providers,
    known: &Observed,
    state: &mut State,
) -> Result<Applied, ProviderError> {
    let mut observed = known.clone();
    let mut changed = 0usize;

    for stage in plan.stages() {
        for change in stage.changes() {
            let address = change.address();
            let provider = providers.for_address(address)?;

            if change.kind() == ChangeKind::Delete {
                if let Some(record) = state.get(address) {
                    provider.delete(address, record.attributes())?;
                }
                state.forget(address);
                changed += 1;
                continue;
            }

            // Destroy before create, and only for a replacement: the resource
            // that is there is in the way of the one that should be, and only
            // state knows where the first one actually is.
            if change.kind() == ChangeKind::Replace
                && let Some(record) = state.get(address)
            {
                provider.delete(address, record.attributes())?;
            }

            let attributes = settle(change, &observed);
            let result = provider.apply(address, change.kind(), &attributes)?;
            state.record(
                address.clone(),
                Record::new(result.clone(), dependencies(change)),
            );
            observed.record(address.clone(), result);
            changed += 1;
        }
    }

    ensures!(
        changed == plan.changes().count(),
        "every change in the plan was carried out, or the run stopped"
    );
    Ok(Applied { observed, changed })
}

// ENFORCES: INV-PLAN-004
/// What this change read from, so that its removal can be ordered later.
///
/// Taken from the plan rather than from the graph: the plan is what was actually
/// carried out, and the configuration that described it may be gone by the time
/// this is read back.
fn dependencies(change: &Change) -> BTreeSet<Address> {
    change
        .desired()
        .iter()
        .filter_map(|(_, value)| match value {
            Confidence::Unknown(reference) => Some(reference.source().clone()),
            Confidence::Known(_) => None,
        })
        .collect()
}

/// Every attribute of a change, with what was unknown at plan time filled in.
fn settle(change: &Change, observed: &Observed) -> Attributes {
    let mut attributes = Attributes::new();
    for (name, wanted) in change.desired() {
        let value = match wanted {
            Confidence::Known(value) => value.clone(),
            Confidence::Unknown(reference) => {
                let resolved = observed.attribute(reference.source(), reference.attribute());
                requires!(
                    resolved.is_some(),
                    "{} needs {reference}, which its stage should have produced already",
                    change.address()
                );
                match resolved {
                    Some(value) => value.clone(),
                    // Unreachable while contracts are compiled in; without them,
                    // skipping is the only answer that does not invent a value.
                    None => continue,
                }
            }
        };
        attributes.insert(name.clone(), value);
    }
    attributes
}
