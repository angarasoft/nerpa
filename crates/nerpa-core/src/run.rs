//! Reading the world, and making it match.
//!
//! Two walks, and they are not symmetrical. Reading happens once, before
//! anything else, so that the plan is complete before the first action
//! (`0006`). Applying walks the plan in the order the
//! plan already fixed, and resolves as it goes: a value that was unknown at plan
//! time is read out of what an earlier stage produced.
//!
//! Nothing here touches the world itself. It calls providers, which do.

use std::collections::{BTreeMap, BTreeSet};

use crate::address::{Address, Provider as ProviderName, Target};
use crate::attribute::Attribute;
use crate::graph::{Graph, GraphError};
use crate::observed::{Facts, Observed};
use crate::plan::{Change, ChangeKind, Confidence, Plan};
use crate::provider::{
    Answered, Attributes, Concluded, Concluding, Machines, Planned, Prepared, Provider,
    ProviderError, Providers, Reach, Reading, Sending,
};
use crate::resource::Resource;
use crate::secret::{FetchError, Recorded, Secrets};
use crate::state::{Mark, Record, Recording, State};
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
        let locator = match (
            state.get(&address),
            state.marked(&address),
            graph.get(&address),
        ) {
            // Where it is, not where the declaration now says it should be. A
            // resource whose location changed has to be found where it was left,
            // or the change reads as making a second one and abandoning the
            // first.
            // ENFORCES: INV-STATE-003
            (Some(record), _, _) => record.locator(),
            // No record, and a run that stopped left an entry: what that change
            // may have made is where the entry says, which is not where the
            // declaration points once somebody has moved it. The same rule as
            // above — found where it was left — for the one case where state
            // remembers a place without remembering a thing.
            (None, Some(entry), _) => entry.locator().clone(),
            (None, None, Some(resource)) => settled_declaration(resource),
            (None, None, None) => continue,
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

    // Records, and not the entries beside them. An entry whose declaration is
    // gone is removed from what state remembers rather than from a reading, and
    // one still declared is read because it is declared — so looking for the
    // rest would be a journey whose answer nothing asks for.
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
            let seal = address.target().map(|node| secrets.key().seal_for(node));
            let sealing = seal.map(|seal| Sealing {
                machine: reach,
                seal,
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
            let recorded = where_it_is(state, address);

            // A removal of something state does not remember has nothing to
            // remove: state is the only thing that knows where it was.
            if kind == Sending::Delete && recorded.is_none() {
                state.forget(address);
                record(state).map_err(RunError::Record)?;
                changed += 1;
                continue;
            }

            let desired = settled(change, kind, &observed, secrets)?;
            // ENFORCES: INV-REACT-004
            // Told whether it was roused, whatever else changed. A service whose own
            // attributes change in the same plan as its file is one Update, and the
            // provider has to know to restart it as well.
            let roused = !(change.roused_by().is_empty() && change.owed().is_empty());

            let secret = secret_names(change);
            let shown = told(kind, &desired, recorded.as_ref(), &secret);
            let changing = Changing {
                kind,
                roused,
                desired: &desired,
                recorded: recorded.as_ref(),
                facts,
                secret: &secret,
                shown: &shown,
            };

            let planned = marked_and_prepared(provider, change, &changing, state, record)?;
            let (became, confirmed) = match carried_out(
                planned,
                &secret,
                reach,
                change,
                seal.as_ref(),
                state,
                record,
            ) {
                Ok(outcome) => outcome,
                Err(stopped) => return Err(accounted_for(stopped, address, state, record)?),
            };
            // A provider saying the answers show the change did not take is a
            // change whose operations ran all the same.
            let concluded = concluded(provider, change, &changing, &became).map_err(Stopped::Begun);
            let Some(result) = (match concluded {
                Ok(result) => result,
                Err(stopped) => return Err(accounted_for(stopped, address, state, record)?),
            }) else {
                // Forgotten on every `Gone`, whatever the change was: a removal
                // that only settles — a service stopped, its unit left where it
                // was — says it is gone the same way.
                state.forget(address);
                state.settle(address);
                record(state).map_err(RunError::Record)?;
                changed += 1;
                continue;
            };

            wrote(
                state,
                &mut observed,
                change,
                &Reported {
                    result: &result,
                    desired: &desired,
                    confirmed: &confirmed,
                },
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
    // An entry kept only so that what a failed change made could be removed is
    // answered by this: its provider said reading could settle the change, and
    // this is that reading, written down. Left standing, it would outlive the
    // question it was asked and nothing would ever clear it. RFC 0001 §5.6.
    state.settle(address);
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

/// What is known about a change once it has been carried out.
///
/// Three things and not one, because they come from three places: what the
/// provider concluded, what the engine settled and sent, and which attributes a
/// write's checked digest confirmed.
#[derive(Debug)]
struct Reported<'a> {
    /// What `conclude` said the resource became.
    result: &'a Attributes,
    /// What was settled and sent, secrets included.
    desired: &'a Attributes,
    /// What a write's digest confirmed, whether or not `conclude` mentioned it.
    confirmed: &'a BTreeSet<String>,
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
    reported: &Reported<'_>,
    secrets: Secrets<'_>,
    record: &mut dyn FnMut(&State) -> Result<(), String>,
) -> Result<(), RunError> {
    let address = change.address();
    // What a checked write confirmed and `conclude` never reported is supplied
    // from what was sent. A secret is the case that needs it: nothing downstream
    // of sending holds one, so the provider cannot hand it back, and the digest
    // state records has to come from the engine's own copy.
    let mut recorded = reported.result.clone();
    for name in reported.confirmed {
        if recorded.contains_key(name) {
            continue;
        }
        if let Some(value) = reported.desired.get(name) {
            recorded.insert(name.clone(), value.clone());
        }
    }

    // A fresh record owes nothing, which is how a debt written before its cause
    // was sent is cleared: paid by having got this far.
    state.record(
        address.clone(),
        Record::new(recording(&recorded, change, secrets), dependencies(change)),
    );
    // Settled in the same write as the record it settles: a mark that outlived
    // its answer would hold this machine for a change whose outcome is known.
    state.settle(address);
    observed.record(address.clone(), recorded.clone());
    record(state).map_err(RunError::Record)?;

    if let Some(name) = reported
        .desired
        .keys()
        .find(|name| !recorded.contains_key(*name))
    {
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
) -> Result<Vec<nerpa_proto::Seen>, Stopped> {
    let address = change.address();
    let Some(forgetting) = planned.forget_after else {
        return perform(reach, planned.operations, change.premise(), address);
    };

    requires!(
        forgetting <= planned.operations.len(),
        "{address} was prepared as forgotten after {forgetting} operations, and has {}",
        planned.operations.len()
    );
    let (removing, making) = planned.operations.split_at(forgetting);
    let mut became = perform(reach, removing.to_vec(), change.premise(), address)?;
    state.forget(address);
    // The half that removes has run by now, so a state that cannot be written
    // is a machine something has already happened to.
    record(state).map_err(|problem| Stopped::Begun(RunError::Record(problem)))?;
    // The premise describes the world before this change, and this change has
    // just altered it: what it names is what was removed a moment ago. Checking
    // it again would refuse the run for a difference this run made on purpose.
    became.extend(perform(reach, making.to_vec(), &[], address)?);
    Ok(became)
}

/// What this change is sending, settled: nothing for a removal, and everything
/// the declaration asks for otherwise.
///
/// # Errors
///
/// Returns [`RunError::Unsettled`] for a value that depends on a fact nobody
/// read, and [`RunError::Secret`] for one that could not be fetched.
fn settled(
    change: &Change,
    kind: Sending,
    observed: &Observed,
    secrets: Secrets<'_>,
) -> Result<Attributes, RunError> {
    if kind == Sending::Delete {
        return Ok(Attributes::new());
    }
    settle(change, observed, secrets)
}

/// One change, refused or sent — and what came back, checked against what went.
///
/// Returns the answers as a provider is told them, and the declared attributes a
/// checked write confirmed.
///
/// # Errors
///
/// Returns whatever [`withheld`] refuses before sending, and
/// [`RunError::Unconfirmed`] for an answer that is not what the write would have
/// left behind.
#[allow(
    clippy::too_many_arguments,
    reason = "what carrying out one change needs, one argument each"
)]
fn carried_out(
    planned: Planned,
    secret: &BTreeSet<String>,
    reach: &dyn Reach,
    change: &Change,
    seal: Option<&nerpa_proto::Seal>,
    state: &mut State,
    record: &mut dyn FnMut(&State) -> Result<(), String>,
) -> Result<(Vec<Answered>, BTreeSet<String>), Stopped> {
    let address = change.address();
    // Refused before anything is sent, so nothing of it ran.
    withheld(&planned, secret, seal, address).map_err(Stopped::Untouched)?;
    let expecting = expected(&planned, seal);
    let confirmed = confirmed_by(&planned, &expecting);
    // An answer that is not what was sent means the write ran and left something
    // else, which is the one thing nobody may record as though it had not.
    let became = checked(
        send(planned, reach, change, state, record)?,
        &expecting,
        address,
    )
    .map_err(Stopped::Begun)?;
    Ok((became, confirmed))
}

/// What the provider made of the answers to its own change.
///
/// `None` is [`Concluded::Gone`]: the resource is not there any more, whatever
/// the change was — a removal, or something that only settles and says so.
///
/// # Errors
///
/// Returns [`RunError::Provider`] if the answers say the change did not take.
fn concluded(
    provider: &dyn Provider,
    change: &Change,
    changing: &Changing<'_>,
    answers: &[Answered],
) -> Result<Option<Attributes>, RunError> {
    let concluding = Concluding {
        address: change.address(),
        kind: changing.kind,
        roused: changing.roused,
        facts: changing.facts,
        plain: changing.shown,
        secret: changing.secret,
    };
    match provider.conclude(&concluding, answers)? {
        Concluded::Gone => Ok(None),
        Concluded::Became(result) => Ok(Some(result)),
    }
}

/// What each write's answer has to be, if it did what it was sent to do.
///
/// Worked out from what is being sent, before it is sent: the engine holds the
/// bytes and the mode, so it knows what the path will hash to. A write that
/// carries a secret is answered sealed for the machine, so the comparison is
/// between two sealed digests and nothing unkeyed is anywhere.
fn expected(
    planned: &Planned,
    seal: Option<&nerpa_proto::Seal>,
) -> BTreeMap<usize, nerpa_proto::Seen> {
    let mut wanted = BTreeMap::new();
    for (at, operation) in planned.operations.iter().enumerate() {
        let nerpa_proto::Operation::File {
            content,
            mode,
            secret,
            ..
        } = operation
        else {
            continue;
        };
        let digest = nerpa_proto::digest(&nerpa_proto::Seen::Entry(nerpa_proto::Entry::File {
            content: content.clone(),
            mode: *mode,
        }));
        let answer = if *secret {
            // Unreachable: a secret write with no seal is refused before this.
            let Some(seal) = seal else { continue };
            nerpa_proto::Seen::Sealed {
                digest: nerpa_proto::sealed(seal, &digest),
            }
        } else {
            nerpa_proto::Seen::Digest { digest }
        };
        wanted.insert(at, answer);
    }
    wanted
}

/// The declared attributes a matching write confirms.
fn confirmed_by(
    planned: &Planned,
    expecting: &BTreeMap<usize, nerpa_proto::Seen>,
) -> BTreeSet<String> {
    expecting
        .keys()
        .filter_map(|at| planned.carries.get(at))
        .flatten()
        .cloned()
        .collect()
}

/// What the far end said, checked against what it was sent.
///
/// A write whose digest matches becomes [`Answered::Written`], which is a
/// provider being told that the write took rather than being handed the bytes
/// again. One that does not match is a failure after the operations ran: the
/// path holds something other than what was sent, and nothing downstream may
/// record it as though it did.
///
/// # Errors
///
/// Returns [`RunError::Unconfirmed`] for an answer that is not what the write
/// would have left behind.
fn checked(
    became: Vec<nerpa_proto::Seen>,
    expecting: &BTreeMap<usize, nerpa_proto::Seen>,
    address: &Address,
) -> Result<Vec<Answered>, RunError> {
    became
        .into_iter()
        .enumerate()
        .map(|(at, seen)| match expecting.get(&at) {
            None => Ok(Answered::Saw(seen)),
            Some(wanted) if *wanted == seen => Ok(Answered::Written),
            Some(_) => Err(RunError::Unconfirmed {
                address: address.clone(),
                at,
            }),
        })
        .collect()
}

/// What a change may not be sent with: a secret nothing carries, and a secret
/// carried by an operation that does not say it is carrying one.
///
/// # Errors
///
/// Returns [`RunError::Uncarried`] or [`RunError::Unflagged`], both of them
/// before anything is sent.
fn withheld(
    planned: &Planned,
    secret: &BTreeSet<String>,
    seal: Option<&nerpa_proto::Seal>,
    address: &Address,
) -> Result<(), RunError> {
    if seal.is_none()
        && planned
            .operations
            .iter()
            .any(|operation| matches!(operation, nerpa_proto::Operation::File { secret: true, .. }))
    {
        return Err(RunError::Unsealed {
            address: address.clone(),
        });
    }
    if let Some(attribute) = uncarried(planned, secret) {
        return Err(RunError::Uncarried {
            address: address.clone(),
            attribute,
        });
    }
    if let Some(attribute) = unflagged(planned, secret) {
        return Err(RunError::Unflagged {
            address: address.clone(),
            attribute,
        });
    }
    Ok(())
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

/// The mark for a change about to be sent.
///
/// It carries where the change believes the resource is, what it settled that
/// was not a secret, what it comes after, and what its provider says the next
/// run could do about it — asked now rather than later, because the provider
/// that answered may be gone by the time anybody reads this.
fn marking(change: &Change, changing: &Changing<'_>, provider: &dyn Provider) -> Mark {
    Mark::new(
        change.kind(),
        // Where state says it is; and for a change with no record yet, where the
        // declaration says it will be. Either way the next run has somewhere to
        // look for what this change may have made.
        changing
            .recorded
            .cloned()
            .unwrap_or_else(|| changing.shown.clone()),
        changing.shown.clone(),
    )
    .after(dependencies(change))
    .afterwards(provider.afterwards(change.address(), changing.kind))
}

/// A run stopping, and whether anything on the machine had begun.
///
/// The difference is what a mark becomes. Nothing ran and the mark goes, because
/// it stood for a request that did not happen; something ran and it stays, as an
/// entry, because something is out there that nobody has accounted for.
#[derive(Debug)]
enum Stopped {
    /// Nothing was sent, or nothing the far end was asked to do began.
    Untouched(RunError),
    /// Operations ran before it stopped.
    Begun(RunError),
}

/// Settles the mark of a change that stopped, and says what stopped it.
///
/// ENFORCES: INV-STATE-005
/// Written down before the failure is reported, because the report is what ends
/// the run: a mark left saying "in flight" by a run that is over would hold its
/// machine for ever, and an entry never written would lose what the far end may
/// have made.
///
/// # Errors
///
/// Returns [`RunError::Record`] if the settling could not be written; otherwise
/// the failure it was given.
fn accounted_for(
    stopped: Stopped,
    address: &Address,
    state: &mut State,
    record: &mut dyn FnMut(&State) -> Result<(), String>,
) -> Result<RunError, RunError> {
    let (error, began) = match stopped {
        Stopped::Untouched(error) => (error, false),
        Stopped::Begun(error) => (error, true),
    };

    match (state.settle(address), began) {
        // Something ran. What the mark stood for becomes what the next run has
        // to deal with: sent again where reading cannot tell whether it took,
        // and kept only to remove by where it can.
        (Some(mark), true) => {
            state.mark(address.clone(), mark.settled());
        }
        // Nothing ran. What was there before this run's mark comes back, so an
        // entry an earlier run left is not lost by a run that never sent
        // anything.
        (Some(mark), false) => {
            if let Some(replaced) = mark.replaced() {
                state.mark(address.clone(), replaced.clone());
            }
        }
        (None, _) => {}
    }

    record(state).map_err(RunError::Record)?;
    Ok(error)
}

/// What the engine has settled about one change before it is sent.
///
/// Seven things that travel together from the moment a change is settled until
/// its answer is written down, and that a provider is handed in two different
/// shapes along the way. Carried as one so that the loop says what happens
/// rather than repeating what is already known.
struct Changing<'a> {
    /// What is happening to the resource.
    kind: Sending,
    /// Whether something it reacts to changed.
    roused: bool,
    /// Every settled value, secrets included.
    desired: &'a Attributes,
    /// What state recorded, where it recorded anything.
    recorded: Option<&'a Attributes>,
    /// What the machine said about itself.
    facts: Facts<'a>,
    /// Which of the settled values are secrets, by name.
    secret: &'a BTreeSet<String>,
    /// The settled values that are not secrets.
    shown: &'a Attributes,
}

/// Writes the mark, then asks the provider what to send.
///
/// ENFORCES: INV-STATE-005
/// In this order, and durably, because a request sent without a mark is a change
/// nothing records: the run that stops in between leaves a machine that was
/// acted on and a state file saying nothing happened. A provider that then
/// refuses to prepare has sent nothing, so the mark goes again — a mark stands
/// for a request that may have gone out, and this one certainly did not.
///
/// # Errors
///
/// Returns [`RunError::Record`] if the mark could not be written down, and
/// [`RunError::Provider`] if the change could not be prepared.
fn marked_and_prepared(
    provider: &dyn Provider,
    change: &Change,
    changing: &Changing<'_>,
    state: &mut State,
    record: &mut dyn FnMut(&State) -> Result<(), String>,
) -> Result<Planned, RunError> {
    let address = change.address();
    state.mark(address.clone(), marking(change, changing, provider));
    record(state).map_err(RunError::Record)?;

    let prepared = Prepared {
        address,
        kind: changing.kind,
        roused: changing.roused,
        // Nothing marks a change as sent again until an entry can say so
        // (RFC 0001 §5.6).
        resent: change.is_resent(),
        desired: changing.desired,
        recorded: changing.recorded,
        facts: changing.facts,
        secret: changing.secret,
    };
    match provider.prepare(&prepared) {
        Ok(planned) => Ok(planned),
        Err(refusal) => {
            state.settle(address);
            record(state).map_err(RunError::Record)?;
            Err(refusal.into())
        }
    }
}

/// What a provider is told when its change is over, and which of its attributes
/// were secrets.
///
/// No secret value among it: a secret went into the operation that carries it
/// and stops there. A removal is told what state recorded, because the
/// declaration that described it is gone.
/// Where state says it is; and, where there is no record, where the entry of a
/// change that never got one says it may be.
///
/// Something that failed after its operations ran is out there somewhere, and
/// its entry is the only thing that knows where to look for it.
fn where_it_is(state: &State, address: &Address) -> Option<Attributes> {
    state
        .get(address)
        .map(Record::locator)
        .or_else(|| state.marked(address).map(|mark| mark.locator().clone()))
}

fn told(
    kind: Sending,
    desired: &Attributes,
    recorded: Option<&Attributes>,
    secret: &BTreeSet<String>,
) -> Attributes {
    if kind == Sending::Delete {
        return recorded.cloned().unwrap_or_default();
    }
    desired
        .iter()
        .filter(|(name, _)| !secret.contains(name.as_str()))
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect()
}

/// Which of a change's attributes the declaration called secrets.
fn secret_names(change: &Change) -> BTreeSet<String> {
    change
        .desired()
        .iter()
        .filter(|(_, wanted)| wanted.is_secret())
        .map(|(name, _)| name.clone())
        .collect()
}

/// The first secret attribute carried by an operation that does not say it is
/// carrying one.
///
/// The provider marks the operation and this checks the marking, so that a
/// provider which forgets is caught by something that did not do the marking.
/// An unmarked write is answered by the far end with what it read back — which,
/// for a secret, is the secret.
fn unflagged(planned: &Planned, secret: &BTreeSet<String>) -> Option<String> {
    for (at, carried) in &planned.carries {
        let Some(name) = secret.iter().find(|name| carried.contains(name.as_str())) else {
            continue;
        };
        let marked = matches!(
            planned.operations.get(*at),
            Some(nerpa_proto::Operation::File { secret: true, .. })
        );
        if !marked {
            return Some(name.clone());
        }
    }
    None
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
) -> Result<Vec<nerpa_proto::Seen>, Stopped> {
    let asked = operations.len();
    if asked == 0 {
        return Ok(Vec::new());
    }
    // A machine that could not be reached at all was not asked to do anything.
    let answer = reach
        .ask(&nerpa_proto::Ask::Perform {
            plan: None,
            premise: premise.to_vec(),
            operations,
            seal: None,
        })
        .map_err(|error| Stopped::Untouched(error.into()))?;
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
) -> Result<Vec<nerpa_proto::Seen>, Stopped> {
    let refused = |problem: String| ProviderError::World {
        address: address.clone(),
        problem,
    };
    match answer {
        nerpa_proto::Answer::Found { seen } | nerpa_proto::Answer::Done { became: seen }
            if seen.len() == asked =>
        {
            Ok(seen)
        }
        // An answer nobody can read by position is an answer that says nothing
        // about what ran, which is not the same as saying nothing ran.
        nerpa_proto::Answer::Found { seen } | nerpa_proto::Answer::Done { became: seen } => {
            Err(Stopped::Begun(
                refused(format!(
                    "the far end answered {} things about {asked} it was asked, and they are \
                     read by position",
                    seen.len()
                ))
                .into(),
            ))
        }
        // The machine is not what the plan was computed from. Premises are
        // checked before anything is done, so nothing was.
        nerpa_proto::Answer::Moved { at } => Err(Stopped::Untouched(
            refused(format!(
                "the machine is not what the plan was computed from, at premise {at}"
            ))
            .into(),
        )),
        nerpa_proto::Answer::Failed { at, problem } => {
            let stopped = match at {
                // Refused for what the request was, or stopped while checking
                // what it assumed: nothing of it ran.
                nerpa_proto::At::Before
                | nerpa_proto::At::Premise(_)
                | nerpa_proto::At::Observation(_) => Stopped::Untouched,
                // It got as far as carrying something out — and a place this
                // build does not know is a place it cannot claim nothing
                // happened at.
                _ => Stopped::Begun,
            };
            Err(stopped(refused(format!("{at}: {problem}")).into()))
        }
        other => Err(Stopped::Begun(
            refused(format!("the far end said something unexpected: {other:?}")).into(),
        )),
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
    /// A write's answer is not what it would have left behind.
    ///
    /// The engine holds what it sent and knows what the path must hash to. An
    /// answer that is anything else means the path holds something other than
    /// what was written there, and recording it as written would be a record of
    /// something nobody made.
    #[error("{address}: what came back from the write at {at} is not what it was sent")]
    Unconfirmed {
        /// The resource it was declared on.
        address: Address,
        /// Which operation of the change.
        at: usize,
    },
    /// A write carrying a secret has no seal to answer with.
    ///
    /// Its answer is a digest sealed for the machine, and a resource with no
    /// machine has no seal. Refused rather than sent unsealed, which is what
    /// `INV-PREMISE-002` is about.
    #[error(
        "{address}: a write here carries a secret, and there is no machine to seal its answer for"
    )]
    Unsealed {
        /// The resource it was declared on.
        address: Address,
    },
    /// A secret is carried by an operation that does not say it is carrying one.
    ///
    /// A write marked secret is answered with a sealed digest; an unmarked one is
    /// answered with what the far end read back, which for a secret is the
    /// secret. The provider marks it and this is the other claim: they have to
    /// line up.
    #[error(
        "{address}: {attribute} is a secret, and what carries it does not say so, \
         so the machine would answer with the value"
    )]
    Unflagged {
        /// The resource it was declared on.
        address: Address,
        /// The secret attribute an unmarked operation carries.
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
