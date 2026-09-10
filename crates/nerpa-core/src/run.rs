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
use crate::provider::{Attributes, Machines, ProviderError, Providers, Reading};
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
pub fn observe(
    graph: &Graph,
    providers: &Providers,
    machines: &Machines,
    state: &State,
) -> Result<Observed, RunError> {
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

    // Before the resources, not after. A provider deciding how to ask a
    // machine needs to know what the machine is — `dnf` or `emerge` is a
    // fact, not a guess — and a machine that will not answer is better
    // discovered here than in a journey that was doomed. Asked once per
    // machine, in a journey of its own. Every fact is a question
    // somebody wrote down, so what this costs is the length of that list rather
    // than the length of what a machine could be asked. See decision 0029.
    for node in graph.inventory().nodes() {
        let Ok(reach) = machines.reach(Some(node)) else {
            observed.unreachable(node.clone());
            continue;
        };
        match reach.ask(&nerpa_proto::Ask::Look {
            observations: crate::facts::ASKED
                .iter()
                .map(|asked| (asked.ask)())
                .collect(),
        }) {
            Ok(nerpa_proto::Answer::Found { seen }) => {
                for (asked, answer) in crate::facts::ASKED.iter().zip(seen) {
                    if let Some(said) = (asked.read)(&answer) {
                        observed.learn(node.clone(), asked.name, said);
                    }
                }
            }
            // A machine that will not say what it is is a machine nothing can
            // be planned for, and that is the same fact as one that did not
            // answer — whatever the reason. This is the one place where every
            // failure becomes unreachability rather than only the one that
            // already is, because the subject is a machine rather than a
            // resource on it.
            // ENFORCES: INV-REACH-002
            Ok(_) | Err(_) => observed.unreachable(node.clone()),
        }
    }

    for ((provider_name, node), batch) in batches {
        let provider = providers.named(&provider_name)?;
        // What the machine turned out to be, for a provider that has to decide
        // how to ask it. Empty in the control domain, which has no machine.
        let facts = observed.facts_of(node.as_ref());
        // One journey per machine per phase, and the way there is decided here
        // rather than by the provider: provider logic runs on this machine and
        // the work happens on that one. See decision 0032.
        let reach = match machines.reach(node.as_ref()) {
            Ok(reach) => reach,
            // ENFORCES: INV-REACH-002
            Err(ProviderError::Unreachable { node, .. }) => {
                observed.unreachable(node);
                continue;
            }
            Err(other) => return Err(other.into()),
        };
        match provider.read(&batch, facts, reach) {
            Ok(found) => {
                for (address, premise) in found.premise().clone() {
                    observed.looked_at(address, premise);
                }
                for (address, attributes) in found.attributes().clone() {
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
            // Nor is a value that depends on what the machine turned out to be:
            // finding a resource requires knowing where it is, and that is
            // exactly what is not settled yet.
            Attribute::Unresolved(_) | Attribute::Secret(_) | Attribute::Chosen(_) => None,
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
    machines: &Machines,
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
            let reach = machines.reach(address.target())?;
            // What the machine turned out to be, for a provider whose command
            // depends on it. Read in the same run, before anything was applied.
            let facts = observed.facts_of(address.target());

            if change.kind() == ChangeKind::Delete {
                if let Some(record) = state.get(address) {
                    provider.delete(address, &record.locator(), change.premise(), facts, reach)?;
                }
                state.forget(address);
                record(state).map_err(RunError::Record)?;
                changed += 1;
                continue;
            }

            // Destroy before create, and only for a replacement: the resource
            // that is there is in the way of the one that should be, and only
            // state knows where the first one actually is.
            //
            // ENFORCES: INV-STATE-005
            // The destruction is a change like any other, so it is written down
            // before anything else is attempted. Creating the replacement can
            // fail — no quota, no address, no image — and until this was here
            // that failure left state saying where the old one was, for a
            // resource that had already been destroyed. Every later plan then
            // reasoned from that.
            // The locator is taken out first because `record` here is the
            // callback that writes state down, and a `Record` borrowed from
            // state would shadow it — which is how the write came to be missing.
            let mut premise = change.premise();
            if change.kind() == ChangeKind::Replace
                && let Some(locator) = state.get(address).map(Record::locator)
            {
                provider.delete(address, &locator, premise, facts, reach)?;
                state.forget(address);
                record(state).map_err(RunError::Record)?;
                // The premise describes the world before this change, and this
                // change has just altered it: the thing it names is the thing
                // that was deleted a line ago. Checking it again would refuse
                // the run for a difference this run made on purpose.
                premise = &[];
            }

            // ENFORCES: INV-STATE-005
            // Before the action, not after. A debt written afterwards is lost
            // by exactly the run that failed to pay it, which leaves a new file,
            // an old process and nothing recording that fact — the state 0026
            // exists to make unreachable. The window between this write and the
            // record below resolves toward reporting a debt that was in fact
            // paid: one redundant restart against an outage nobody knows about.
            if !change.roused_by().is_empty() {
                state.owe(address, change.roused_by());
                record(state).map_err(RunError::Record)?;
            }

            let attributes = settle(change, &observed, secrets)?;
            let result =
                provider.apply(address, change.kind(), &attributes, premise, facts, reach)?;

            // A fresh record owes nothing, which is how a debt written above is
            // cleared: paid by having got this far.
            state.record(
                address.clone(),
                Record::new(recording(&result, change, secrets), dependencies(change)),
            );
            observed.record(address.clone(), result.clone());
            // Written down before the next resource is touched. State used to be
            // saved once, after this whole loop returned, so anything that
            // stopped it — a contract violation panicking by design, a machine
            // going away, somebody's Ctrl-C — left every resource this run had
            // already made with nothing recording that it exists. They were not
            // rolled back; they were forgotten.
            // ENFORCES: INV-STATE-005
            record(state).map_err(RunError::Record)?;

            // Checked after the record, not before it. The resource has been
            // made by now, and refusing without writing that down would leave it
            // owned by nobody — the failure this whole loop was rearranged to
            // stop.
            //
            // Reporting an attribute back is the provider's claim that it took
            // it. One that goes in and does not come out never settles: the next
            // plan finds it missing from the reading, proposes the same change,
            // and every run after that does the same for ever. Measured on a
            // file declared with an attribute the `os` provider does not own —
            // it applied cleanly and re-planned until somebody noticed.
            //
            // A provider that cannot read a value back — a credential the cloud
            // will not return — echoes it, and the echo is the claim. What it
            // may not do is say nothing.
            // ENFORCES: INV-PLAN-005
            if let Some(name) = attributes.keys().find(|name| !result.contains_key(*name)) {
                return Err(RunError::Unreported {
                    address: address.clone(),
                    attribute: name.clone(),
                });
            }
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
            // A fact is not a resource, so nothing was created to depend on. It
            // orders nothing, which is the point of it not being a reference.
            Confidence::Known(_) | Confidence::Secret(_) | Confidence::Bounded(_) => None,
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
    /// A provider was given an attribute and did not report it back.
    ///
    /// Almost always an attribute the provider does not own — a typo, or a
    /// declaration written for a different kind. It cannot be let through: an
    /// attribute nothing reports never settles, so the plan proposes it again on
    /// every run and the tool stops converging, quietly.
    #[error(
        "{address}: {attribute} was applied and not reported back, so it would \
         never settle; the provider does not appear to own it"
    )]
    Unreported {
        /// The resource it was declared on.
        address: Address,
        /// The attribute that went in and did not come out.
        attribute: String,
    },
    /// A value that depends on a fact was still open when its stage ran.
    ///
    /// The machine was not looked at, so the branch cannot be taken. Guessing at
    /// one would configure a machine as though it were another, which is the
    /// failure the closed set exists to prevent.
    #[error("{address}: {attribute} waits on {fact}, which was never read")]
    Unsettled {
        /// The resource whose attribute is open.
        address: Address,
        /// Which attribute.
        attribute: String,
        /// What it is waiting on.
        fact: String,
    },
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
) -> Result<Attributes, RunError> {
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
            // Still open when the plan was made, and nothing since has settled
            // it: the machine was not looked at, so the branch cannot be taken.
            // Guessing at one would configure a machine as though it were
            // another.
            Confidence::Bounded(choice) => {
                return Err(RunError::Unsettled {
                    address: change.address().clone(),
                    attribute: name.clone(),
                    fact: choice.fact().to_string(),
                });
            }
        };
        attributes.insert(name.clone(), value);
    }
    Ok(attributes)
}
