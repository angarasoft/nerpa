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
use crate::provider::{
    Answered, Attributes, Concluded, Concluding, Machines, Planned, Prepared, ProviderError,
    Providers, Reach, Reading, Sending,
};
use crate::resource::Resource;
use crate::secret::{FetchError, Recorded, Secrets};
use crate::state::{Record, Recording, State};
use crate::{ensures, requires};

/// One journey's worth: who is asked, where they have to go, and about what.
type Batch = ((ProviderName, Option<Target>), Vec<Reading>);

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
    // One machine at a time was one machine at a time: three of them cost three
    // connect timeouts in a row, and the number a fleet has is not three. Fanned
    // out over a bounded pool, in the order the inventory names them so that the
    // answers do not reorder themselves by which machine was quickest.
    let nodes: Vec<Target> = graph.inventory().nodes().cloned().collect();
    let learned = crate::fanout::every(&nodes, |node| {
        let Ok(reach) = machines.reach(Some(node)) else {
            return Err(node.clone());
        };
        let asking: Vec<Vec<nerpa_proto::Observation>> = crate::facts::ASKED
            .iter()
            .map(|asked| (asked.ask)())
            .collect();
        match reach.ask(&nerpa_proto::Ask::Look {
            observations: asking.iter().flatten().cloned().collect(),
            seal: None,
        }) {
            Ok(nerpa_proto::Answer::Found { seen }) => {
                let mut answered = seen.into_iter();
                let mut said = Vec::new();
                for (asked, wanted) in crate::facts::ASKED.iter().zip(&asking) {
                    let mine: Vec<nerpa_proto::Seen> =
                        answered.by_ref().take(wanted.len()).collect();
                    if let Some(answer) = (asked.read)(&mine) {
                        said.push((asked.name, answer));
                    }
                }
                Ok(said)
            }
            // A machine that will not say what it is is a machine nothing can
            // be planned for, and that is the same fact as one that did not
            // answer — whatever the reason. This is the one place where every
            // failure becomes unreachability rather than only the one that
            // already is, because the subject is a machine rather than a
            // resource on it.
            // ENFORCES: INV-REACH-002
            Ok(_) | Err(_) => Err(node.clone()),
        }
    });

    for (node, answer) in nodes.iter().zip(learned) {
        match answer {
            Ok(said) => {
                for (name, value) in said {
                    observed.learn(node.clone(), name, value);
                }
            }
            Err(node) => observed.unreachable(node),
        }
    }

    // Fanned out the same way, and for the same reason: a batch belongs to one
    // machine, so two batches are two conversations that need not wait for each
    // other. Reading `observed` happens before, writing to it after — the pool
    // hands back answers rather than a shared thing to write into.
    let work: Vec<Batch> = batches.into_iter().collect();
    let read = crate::fanout::every(&work, |((provider_name, node), batch)| {
        let provider = providers.named(provider_name)?;
        // What the machine turned out to be, for a provider that has to decide
        // how to ask it. Empty in the control domain, which has no machine.
        let facts = observed.facts_of(node.as_ref());
        // One journey per machine per phase, and the way there is decided here
        // rather than by the provider: provider logic runs on this machine and
        // the work happens on that one. See decision 0032.
        let reach = machines.reach(node.as_ref())?;
        provider.read(batch, facts, reach)
    });

    for answer in read {
        match answer {
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

/// A machine's connection, with that machine's seal on everything that needs one.
///
/// Between a provider and the machine rather than inside either. A provider
/// builds the batch and has no business holding a key, and the plan cannot carry
/// the seal without putting it in the JSON it exists to keep digests out of.
/// `Seal` prints as nothing, so deriving `Debug` shows the machine and no key.
#[derive(Debug)]
struct Sealing<'a> {
    machine: &'a dyn Reach,
    seal: nerpa_proto::Seal,
}

impl Reach for Sealing<'_> {
    fn ask(&self, ask: &nerpa_proto::Ask) -> Result<nerpa_proto::Answer, ProviderError> {
        let mut sealed = ask.clone();
        if let nerpa_proto::Ask::Perform { seal, .. } | nerpa_proto::Ask::Look { seal, .. } =
            &mut sealed
        {
            *seal = Some(self.seal);
        }
        self.machine.ask(&sealed)
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
    let mut adopted = false;
    let rousing = roused_by_each(plan);

    for stage in plan.stages() {
        for change in stage.changes() {
            let address = change.address();

            // ENFORCES: INV-STATE-006
            // Found as declared, and nothing recorded it — or recorded with other
            // edges than it has now, which is a reorder. Nothing is sent: the
            // machine already is what the plan says, so no provider and no
            // connection are wanted, and what is written down is what was read
            // there — secrets reduced to digests, as for everything else.
            //
            // Not written on its own. An adoption made nothing, so a run that
            // stops before it is written loses nothing: the next plan finds the
            // same thing as declared and adopts it again. It goes with whatever
            // is written next, or once at the end, because a first run over
            // machines already configured adopts everything it declares, and a
            // whole state file for each would be thousands of writes saying that
            // nothing happened.
            if !change.kind().sends_anything() {
                adopted |= adopt(state, &observed, change, secrets);
                changed += 1;
                continue;
            }

            let provider = providers.for_address(address)?;
            let reach = machines.reach(address.target())?;
            // ENFORCES: INV-PREMISE-002
            // The seal goes on here, where the machine is known, and not in the
            // plan, where it would travel with the JSON. Providers never see it:
            // they build the batch, and this seals it as it leaves. Decision 0041.
            let sealing = address.target().map(|node| Sealing {
                machine: reach,
                seal: secrets.key().seal_for(node),
            });
            let reach: &dyn Reach = match &sealing {
                Some(sealing) => sealing,
                None => reach,
            };
            // What the machine turned out to be, for a provider whose command
            // depends on it. Read in the same run, before anything was applied.
            let facts = observed.facts_of(address.target());

            owe(state, change, &rousing, &observed, secrets, record)?;

            let Some(kind) = Sending::of(change.kind()) else {
                // Adoptions and reorders left this loop a few lines above.
                requires!(
                    false,
                    "{address} was planned as sending something, and its kind sends nothing"
                );
                continue;
            };
            let recorded = state.get(address).map(Record::locator);

            // A removal of something state does not remember has nothing to
            // remove: state is the only thing that knows where it was.
            if kind == Sending::Delete && recorded.is_none() {
                state.forget(address);
                record(state).map_err(RunError::Record)?;
                changed += 1;
                continue;
            }

            let desired = if kind == Sending::Delete {
                Attributes::new()
            } else {
                settle(change, &observed, secrets)?
            };
            // ENFORCES: INV-REACT-004
            // Told whether it was roused, whatever else changed. A service whose own
            // attributes change in the same plan as its file is one Update, and the
            // provider has to know to restart it as well.
            let roused = !(change.roused_by().is_empty() && change.owed().is_empty());

            let planned = provider.prepare(&Prepared {
                address,
                kind,
                roused,
                // Nothing marks a change as sent again until state carries marks.
                resent: false,
                desired: &desired,
                recorded: recorded.as_ref(),
                facts,
            })?;

            let (shown, secret) = told(change, kind, &desired, recorded);
            if let Some(attribute) = uncarried(&planned, &secret) {
                return Err(RunError::Uncarried {
                    address: address.clone(),
                    attribute,
                });
            }
            let became = send(planned, reach, change, state, record)?;
            let result = match provider.conclude(
                &Concluding {
                    address,
                    kind,
                    roused,
                    facts,
                    plain: &shown,
                    secret: &secret,
                },
                &became,
            )? {
                // Forgotten on every `Gone`, whatever the change was: a removal
                // that only settles — a service stopped, its unit left where it
                // was — says it is gone the same way.
                Concluded::Gone => {
                    state.forget(address);
                    record(state).map_err(RunError::Record)?;
                    changed += 1;
                    continue;
                }
                Concluded::Became(result) => result,
            };

            wrote(
                state,
                &mut observed,
                change,
                &result,
                &desired,
                secrets,
                record,
            )?;
            changed += 1;
        }
    }

    // Adoptions nothing was written after. One more write when something was,
    // which is cheaper than keeping count of which.
    if adopted {
        record(state).map_err(RunError::Record)?;
    }

    ensures!(
        changed == plan.changes().count(),
        "every change in the plan was carried out, or the run stopped"
    );
    Ok(Applied { observed, changed })
}

/// Writes down something that already matches its declaration.
///
/// Nothing is sent, so there is nothing to conclude from: what is written down
/// is what was read there, secrets reduced to digests as for everything else.
/// Says whether it wrote anything, because the write itself waits — see
/// `INV-STATE-006` where this is called.
fn adopt(state: &mut State, observed: &Observed, change: &Change, secrets: Secrets<'_>) -> bool {
    let address = change.address();
    let found = observed.get(address);
    requires!(
        found.is_some(),
        "{address} was planned as found, and this run was handed a reading without it"
    );
    let Some(found) = found else {
        return false;
    };
    state.record(
        address.clone(),
        Record::new(recording(found, change, secrets), dependencies(change)),
    );
    true
}

/// Writes down what this change rouses, before the change is sent.
///
/// ENFORCES: INV-REACT-002
/// Owed before its cause is sent, not when the reaction's own stage comes.
/// Written there, a debt was lost by exactly the run that stopped in between — a
/// machine gone, a Ctrl-C, a failure in a stage between the two — which left a
/// new file, an old process and nothing recording that fact: the state 0026
/// exists to make unreachable. The window between this write and the reaction
/// resolves toward reporting a debt that was in fact paid: one redundant
/// restart, against an outage nobody knows about.
///
/// ENFORCES: INV-REACT-006
/// And on the records of reacting resources this plan cannot reach, which the
/// planner found to have one. Paid in whichever later run their machine answers
/// in.
fn owe(
    state: &mut State,
    change: &Change,
    rousing: &BTreeMap<&Address, Vec<&Change>>,
    observed: &Observed,
    secrets: Secrets<'_>,
    record: &mut dyn FnMut(&State) -> Result<(), String>,
) -> Result<(), RunError> {
    let address = change.address();
    if let Some(reactors) = rousing.get(address) {
        for reactor in reactors {
            owe_before(state, reactor, address, observed, secrets);
        }
    }
    for reactor in change.owed_by() {
        requires!(
            state.get(reactor).is_some(),
            "{reactor} was planned as owing a reaction on its record, and this run was handed a state without one"
        );
        state.owe(reactor, std::slice::from_ref(address));
    }
    if rousing.contains_key(address) || !change.owed_by().is_empty() {
        record(state).map_err(RunError::Record)?;
    }
    Ok(())
}

/// Writes down what the change became, and refuses what it did not report back.
///
/// ENFORCES: INV-STATE-005
/// Written down before the next resource is touched. State used to be saved
/// once, after the whole loop returned, so anything that stopped it — a contract
/// violation panicking by design, a machine going away, somebody's Ctrl-C — left
/// every resource this run had already made with nothing recording that it
/// exists. They were not rolled back; they were forgotten.
///
/// The refusal comes after the record, not before it. The resource has been made
/// by now, and refusing without writing that down would leave it owned by
/// nobody — the failure this loop was rearranged to stop.
///
/// ENFORCES: INV-PLAN-005
/// Reporting an attribute back is the provider's claim that it took it. One that
/// goes in and does not come out never settles: the next plan finds it missing
/// from the reading, proposes the same change, and every run after that does the
/// same for ever. Measured on a file declared with an attribute the `os`
/// provider does not own — it applied cleanly and re-planned until somebody
/// noticed. A provider that cannot read a value back — a credential the cloud
/// will not return — echoes it, and the echo is the claim. What it may not do is
/// say nothing.
///
/// # Errors
///
/// Returns [`RunError::Record`] if state could not be written, and
/// [`RunError::Unreported`] for an attribute the provider was given and did not
/// report back.
fn wrote(
    state: &mut State,
    observed: &mut Observed,
    change: &Change,
    result: &Attributes,
    desired: &Attributes,
    secrets: Secrets<'_>,
    record: &mut dyn FnMut(&State) -> Result<(), String>,
) -> Result<(), RunError> {
    let address = change.address();
    // A fresh record owes nothing, which is how a debt written before its cause
    // was sent is cleared: paid by having got this far.
    state.record(
        address.clone(),
        Record::new(recording(result, change, secrets), dependencies(change)),
    );
    observed.record(address.clone(), result.clone());
    record(state).map_err(RunError::Record)?;

    if let Some(name) = desired.keys().find(|name| !result.contains_key(*name)) {
        return Err(RunError::Unreported {
            address: address.clone(),
            attribute: name.clone(),
        });
    }
    Ok(())
}

/// One change, sent: one batch, or two where a replacement forgets in between.
///
/// ENFORCES: INV-STATE-005
/// A replacement is two halves of one batch, and the record is dropped between
/// them: in between, the old one is gone and the new one is not there yet.
/// Making the replacement can fail — no quota, no address, no image — and until
/// this was here that failure left state saying where the old one was, for a
/// resource that had already been destroyed. Every later plan then reasoned from
/// that.
fn send(
    planned: Planned,
    reach: &dyn Reach,
    change: &Change,
    state: &mut State,
    record: &mut dyn FnMut(&State) -> Result<(), String>,
) -> Result<Vec<Answered>, RunError> {
    let address = change.address();
    let Some(forgetting) = planned.forget_after else {
        return Ok(perform(
            reach,
            planned.operations,
            change.premise(),
            address,
        )?);
    };

    requires!(
        forgetting <= planned.operations.len(),
        "{address} was prepared as forgotten after {forgetting} operations, and has {}",
        planned.operations.len()
    );
    let (removing, making) = planned.operations.split_at(forgetting);
    let mut became = perform(reach, removing.to_vec(), change.premise(), address)?;
    state.forget(address);
    record(state).map_err(RunError::Record)?;
    // The premise describes the world before this change, and this change has
    // just altered it: what it names is what was removed a moment ago. Checking
    // it again would refuse the run for a difference this run made on purpose.
    became.extend(perform(reach, making.to_vec(), &[], address)?);
    Ok(became)
}

/// The first secret attribute no operation of this change claims to carry.
///
/// `Planned::carries` names, for each operation that writes, the declared
/// attributes a matching answer confirms. A secret outside all of them is a
/// secret sent on trust, and this run does not send one.
fn uncarried(planned: &Planned, secret: &BTreeSet<String>) -> Option<String> {
    secret
        .iter()
        .find(|name| {
            !planned
                .carries
                .values()
                .any(|carried| carried.contains(name.as_str()))
        })
        .cloned()
}

/// What a provider is told when its change is over, and which of its attributes
/// were secrets.
///
/// No secret value among it: a secret went into the operation that carries it
/// and stops there. A removal is told what state recorded, because the
/// declaration that described it is gone.
fn told(
    change: &Change,
    kind: Sending,
    desired: &Attributes,
    recorded: Option<Attributes>,
) -> (Attributes, BTreeSet<String>) {
    let secret: BTreeSet<String> = change
        .desired()
        .iter()
        .filter(|(_, wanted)| wanted.is_secret())
        .map(|(name, _)| name.clone())
        .collect();
    let shown = if kind == Sending::Delete {
        recorded.unwrap_or_default()
    } else {
        desired
            .iter()
            .filter(|(name, _)| !secret.contains(name.as_str()))
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect()
    };
    (shown, secret)
}

// ENFORCES: INV-PROVIDER-002
/// One batch, sent, with an answer for each operation in it.
///
/// Nothing is sent for a change that has no operations: a provider may conclude
/// from what it was told without asking the machine anything.
fn perform(
    reach: &dyn Reach,
    operations: Vec<nerpa_proto::Operation>,
    premise: &[nerpa_proto::Premise],
    address: &Address,
) -> Result<Vec<Answered>, ProviderError> {
    let asked = operations.len();
    if asked == 0 {
        return Ok(Vec::new());
    }
    let answer = reach.ask(&nerpa_proto::Ask::Perform {
        plan: None,
        premise: premise.to_vec(),
        operations,
        seal: None,
    })?;
    answers(answer, address, asked)
}

/// What the far end said about each operation, counted.
///
/// Answers are read by position: one too few and the last operation is described
/// by nothing, one too many and each is described by its neighbour's.
fn answers(
    answer: nerpa_proto::Answer,
    address: &Address,
    asked: usize,
) -> Result<Vec<Answered>, ProviderError> {
    match answer {
        nerpa_proto::Answer::Found { seen } | nerpa_proto::Answer::Done { became: seen }
            if seen.len() == asked =>
        {
            Ok(seen.into_iter().map(Answered::Saw).collect())
        }
        nerpa_proto::Answer::Found { seen } | nerpa_proto::Answer::Done { became: seen } => {
            Err(ProviderError::World {
                address: address.clone(),
                problem: format!(
                    "the far end answered {} things about {asked} it was asked, and they are \
                     read by position",
                    seen.len()
                ),
            })
        }
        nerpa_proto::Answer::Moved { at } => Err(ProviderError::World {
            address: address.clone(),
            problem: format!("the machine is not what the plan was computed from, at premise {at}"),
        }),
        nerpa_proto::Answer::Failed { at, problem } => Err(ProviderError::World {
            address: address.clone(),
            problem: format!("at {at}: {problem}"),
        }),
        other => Err(ProviderError::World {
            address: address.clone(),
            problem: format!("the far end said something unexpected: {other:?}"),
        }),
    }
}

/// What each change in `plan` rouses, by cause, in plan order.
///
/// Built once, so that the debts a change leaves can be written before it is sent
/// without looking through the rest of the plan for every change.
fn roused_by_each(plan: &Plan) -> BTreeMap<&Address, Vec<&Change>> {
    let mut rousing: BTreeMap<&Address, Vec<&Change>> = BTreeMap::new();
    for later in plan.changes() {
        for cause in later.roused_by() {
            rousing.entry(cause).or_default().push(later);
        }
    }
    rousing
}

/// Writes down that `reactor` owes a reaction to `cause`, before `cause` is sent.
///
/// A reacting resource state does not record yet — found as declared, and roused
/// in the plan that would otherwise have adopted it — is recorded from what was
/// read, so that the debt has somewhere to be written. One that is not there, or
/// is about to be made again, owes nothing: whatever starts it reads the new file.
fn owe_before(
    state: &mut State,
    reactor: &Change,
    cause: &Address,
    observed: &Observed,
    secrets: Secrets<'_>,
) {
    let address = reactor.address();
    if state.get(address).is_some() {
        state.owe(address, std::slice::from_ref(cause));
        return;
    }
    let found = match reactor.kind() {
        ChangeKind::React | ChangeKind::Update => observed.get(address),
        ChangeKind::Create
        | ChangeKind::Replace
        | ChangeKind::Delete
        | ChangeKind::Adopt
        | ChangeKind::Reorder => None,
    };
    if let Some(found) = found {
        state.record(
            address.clone(),
            Record::new(recording(found, reactor, secrets), dependencies(reactor))
                .owing(BTreeSet::from([cause.clone()])),
        );
    }
}

// ENFORCES: INV-PLAN-004
/// What this change came after, so that its removal can be ordered later.
///
/// Taken from the plan rather than from the graph: the plan is what was actually
/// carried out, and the configuration that described it may be gone by the time
/// this is read back. Every edge, however it arose. This used to take only the
/// references still unresolved when planning, so a dependency that already
/// existed, or one written as `after`, was never remembered.
fn dependencies(change: &Change) -> BTreeSet<Address> {
    change.depends_on().iter().cloned().collect()
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
    /// A secret was to be sent, and no operation claims to carry it.
    ///
    /// What a provider says a write carries is what a matching answer confirms.
    /// A secret no write carries would be sent and confirmed by nothing, and
    /// state would record a digest of a value nobody checked had arrived — which
    /// is the one recording that cannot be corrected by looking, because looking
    /// is exactly what a secret does not allow.
    #[error(
        "{address}: {attribute} is a secret, and nothing this change sends says it \
         carries it, so nothing would confirm that it arrived"
    )]
    Uncarried {
        /// The resource it was declared on.
        address: Address,
        /// The secret attribute nothing carries.
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
