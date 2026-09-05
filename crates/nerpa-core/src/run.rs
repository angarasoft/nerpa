//! Reading the world, and making it match.
//!
//! Two walks, and they are not symmetrical. Reading happens once, before
//! anything else, so that the plan is complete before the first action
//! ([0006](../../../docs/decisions)). Applying walks the plan in the order the
//! plan already fixed, and resolves as it goes: a value that was unknown at plan
//! time is read out of what an earlier stage produced.
//!
//! Nothing here touches the world itself. It calls providers, which do.

use std::collections::{BTreeMap, BTreeSet};

use crate::address::{Address, Provider as ProviderName, Target};
use crate::attribute::Attribute;
use crate::graph::{Graph, GraphError};
use crate::observed::Observed;
use crate::plan::{Change, ChangeKind, Confidence, Plan};
use crate::provider::{Attributes, ProviderError, Providers, Reading};
use crate::resource::Resource;
use crate::secret::{FetchError, Recorded, Secrets};
use crate::state::{Record, Recording, State};
use crate::{ensures, requires};

/// Reads every resource the configuration declares.
///
/// # Errors
///
/// Returns [`ProviderError`] if any resource could not be read. A partial
/// reading is not a smaller reading: planning against it would propose
/// creating things that are already there.
// ENFORCES: INV-REACH-001
pub fn observe(graph: &Graph, providers: &Providers, state: &State) -> Result<Observed, RunError> {
    let mut observed = Observed::new();

    // ENFORCES: INV-BATCH-001
    // Grouped before anything is asked, so that one machine takes one journey
    // rather than one per resource. The key is the pair that decides who is
    // asked and where they have to go; a batch never spans either.
    let mut batches: BTreeMap<(ProviderName, Option<Target>), Vec<Reading>> = BTreeMap::new();

    for address in declared_and_remembered(graph, state)? {
        let locator = match (graph.get(&address), state.get(&address)) {
            // Where it is, not where the declaration now says it should be. A
            // resource whose location changed has to be found where it was left,
            // or the change reads as making a second one and abandoning the
            // first.
            // ENFORCES: INV-STATE-003
            (_, Some(record)) => record.locator(),
            (Some(resource), None) => settled_declaration(resource),
            (None, None) => continue,
        };

        let key = (address.provider().clone(), address.target().cloned());
        batches
            .entry(key)
            .or_default()
            .push(Reading::new(address, locator));
    }

    for ((provider_name, _), batch) in batches {
        let provider = providers.named(&provider_name)?;
        match provider.read(&batch) {
            Ok(found) => {
                for (address, attributes) in found {
                    observed.record(address, attributes);
                }
            }
            // ENFORCES: INV-REACH-002
            // The one failure a run survives. Everything else still stops it,
            // because everything else means something is wrong rather than
            // merely out of reach. See decision 0020.
            Err(ProviderError::Unreachable { node, .. }) => observed.unreachable(node),
            Err(other) => return Err(other.into()),
        }
    }

    Ok(observed)
}

/// Every address worth looking at: what the configuration declares, and what
/// state remembers making.
///
/// The second half matters as much as the first. A resource the configuration no
/// longer mentions still has to be read, or its removal would be planned blind.
fn declared_and_remembered(graph: &Graph, state: &State) -> Result<BTreeSet<Address>, GraphError> {
    // Propagated, not flattened away. `stages()` returns a `Result`, and
    // `.into_iter().flatten()` on one yields nothing for an error — so a graph
    // that could not be ordered used to read as a configuration that declares
    // nothing, and the run would go on to look at what state remembered and at
    // nothing else. Fail closed: the graph is the thing being asked about.
    // ENFORCES: INV-GRAPH-005
    let declared = graph
        .stages()?
        .into_iter()
        .flat_map(|stage| stage.addresses().to_vec())
        .collect::<BTreeSet<_>>();

    Ok(declared
        .into_iter()
        .chain(state.addresses().cloned())
        .collect())
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
            // A secret is never a locator: nothing is found by its password, and
            // fetching one in order to look something up would be reaching for a
            // credential to do a job that does not need it.
            Attribute::Unresolved(_) | Attribute::Secret(_) => None,
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
    secrets: Secrets<'_>,
    record: &mut dyn FnMut(&State) -> Result<(), String>,
) -> Result<Applied, RunError> {
    let mut observed = known.clone();
    let mut changed = 0usize;

    for stage in plan.stages() {
        for change in stage.changes() {
            let address = change.address();
            let provider = providers.for_address(address)?;

            if change.kind() == ChangeKind::Delete {
                if let Some(record) = state.get(address) {
                    provider.delete(address, &record.locator())?;
                }
                state.forget(address);
                record(state).map_err(RunError::Record)?;
                changed += 1;
                continue;
            }

            // Destroy before create, and only for a replacement: the resource
            // that is there is in the way of the one that should be, and only
            // state knows where the first one actually is.
            if change.kind() == ChangeKind::Replace
                && let Some(record) = state.get(address)
            {
                provider.delete(address, &record.locator())?;
            }

            let attributes = settle(change, &observed, secrets)?;
            let result = provider.apply(address, change.kind(), &attributes)?;
            state.record(
                address.clone(),
                Record::new(recording(&result, change, secrets), dependencies(change)),
            );
            observed.record(address.clone(), result);
            // Written down before the next resource is touched. State used to be
            // saved once, after this whole loop returned, so anything that
            // stopped it — a contract violation panicking by design, a machine
            // going away, somebody's Ctrl-C — left every resource this run had
            // already made with nothing recording that it exists. They were not
            // rolled back; they were forgotten.
            // ENFORCES: INV-STATE-005
            record(state).map_err(RunError::Record)?;
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
            Confidence::Known(_) | Confidence::Secret(_) => None,
        })
        .collect()
}

/// What state may keep of what a provider reported.
///
/// Values, except where the declaration said the attribute was a secret — those
/// become digests here and are never values again. This is the one place a
/// credential could have been written down, and it is the reason it is not.
// ENFORCES: INV-SECRET-002
fn recording(reported: &Attributes, change: &Change, secrets: Secrets<'_>) -> Recording {
    let secret: BTreeSet<&str> = change
        .desired()
        .iter()
        .filter(|(_, wanted)| wanted.is_secret())
        .map(|(name, _)| name.as_str())
        .collect();

    reported
        .iter()
        .map(|(name, value)| {
            let recorded = if secret.contains(name.as_str()) {
                Recorded::Secret(secrets.digest(value))
            } else {
                Recorded::Plain(value.clone())
            };
            (name.clone(), recorded)
        })
        .collect()
}

/// Why a run stopped.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RunError {
    /// A provider could not do what was asked of it.
    #[error(transparent)]
    Provider(#[from] ProviderError),
    /// A secret a change needed could not be fetched.
    #[error(transparent)]
    Secret(#[from] FetchError),
    /// The configuration could not be ordered, so there is nothing to look at
    /// and nothing to do.
    #[error(transparent)]
    Graph(#[from] GraphError),
    /// What had been done could not be written down.
    ///
    /// Stops the run. Carrying on would make more changes that nothing is
    /// recording, and the gap between the world and the record is the one thing
    /// this tool cannot recover by looking.
    #[error("what had been done could not be recorded: {0}")]
    Record(String),
}

/// Every attribute of a change, with what was unknown at plan time filled in.
///
/// Secrets are fetched here, as late as they can be: between this call and the
/// provider that writes them, and nowhere else.
fn settle(
    change: &Change,
    observed: &Observed,
    secrets: Secrets<'_>,
) -> Result<Attributes, FetchError> {
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
            // The last moment, and the only one: fetched here and handed
            // straight to the provider that writes it.
            Confidence::Secret(sealed) => secrets.fetch(sealed.source())?,
        };
        attributes.insert(name.clone(), value);
    }
    Ok(attributes)
}
