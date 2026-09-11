//! Turning desired and observed state into a plan, in one pass.
//!
//! Expands: 0006
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

use std::collections::{BTreeMap, BTreeSet};

use crate::address::{Address, Target};
use crate::attribute::Attribute;
use crate::graph::{Graph, GraphError, StageKind};
use crate::observed::Observed;
use crate::plan::{
    AttributeDelta, Change, ChangeKind, Confidence, Outdated, Plan, PlanStage, Unreachable,
};
use crate::resource::Resource;
use crate::schema::Schema;
use crate::secret::{FetchError, Recorded, Secrets};
use crate::state::{Record, State, Written};
use crate::value::Value;

/// Why a plan could not be produced at all.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PlanError {
    /// The configuration cannot be ordered: a cycle, a dangling dependency, or a
    /// resource on a node the inventory does not know.
    #[error(transparent)]
    Graph(#[from] GraphError),
    /// A machine said something the configuration wrote no branch for.
    ///
    /// Refused rather than defaulted. A silent default is a machine configured
    /// as though it were a different one, which is worse than a machine that is
    /// not configured.
    #[error("{fact} is {said:?}, and the configuration accounts for no such value")]
    Unaccounted {
        /// Which fact.
        fact: crate::attribute::Fact,
        /// What the machine said.
        said: String,
    },
    /// A secret could not be fetched, and whether it changed cannot be guessed.
    ///
    /// Planning stops rather than reporting a secret as unchanged, which is the
    /// answer that would be believed and is the one that could be wrong.
    #[error(transparent)]
    Secret(#[from] FetchError),
    /// A value too large to carry to a machine in one operation.
    #[error(
        "{address}: {attribute} is {size} bytes, and {most} is the most one operation can carry"
    )]
    TooLarge {
        /// The resource declaring it.
        address: Address,
        /// Which attribute.
        attribute: String,
        /// How large it is.
        size: usize,
        /// The most its provider accepts.
        most: usize,
    },
    /// Two resources on one machine that name the same thing there.
    #[error(
        "{first} and {second} both declare {namespace} {value:?} on {node}, and one thing on a machine is declared once"
    )]
    SameThing {
        /// The resource met first.
        first: Address,
        /// The resource that names the same thing.
        second: Address,
        /// The machine they are both on.
        node: Target,
        /// What kind of name they share.
        namespace: String,
        /// The name, as the second declares it.
        value: String,
    },
    /// A resource of a kind that cannot react was given something to react to.
    #[error(
        "{address} is given something to react to, and its provider says a {kind} cannot react"
    )]
    CannotReact {
        /// The resource.
        address: Address,
        /// Its kind.
        kind: String,
    },
}

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
    secrets: Secrets<'_>,
) -> Result<Plan, PlanError> {
    plan_at(graph, observed, state, schema, secrets, Written::now())
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
    secrets: Secrets<'_>,
    now: Written,
) -> Result<Plan, PlanError> {
    // Once, not once per resource. Asking the graph for a single resource's
    // dependencies rebuilds every edge it has, and this loop asks for every
    // resource.
    let dependencies = graph.dependencies()?;
    one_thing_once(graph, schema)?;
    reactions_allowed(graph, schema)?;

    let mut unreachable = Vec::new();
    let (mut stages, held) = removals(graph, state, observed, now, &mut unreachable);
    // ENFORCES: INV-PLAN-009
    // What those removals take away, by what names it on its machine. A declared
    // resource naming the same thing is made again after the removal rather than
    // read from before it: the reading describes a thing this plan is about to
    // delete. Renaming an address and keeping its path, with no `moved`, used to
    // remove the file and adopt the new resource from that reading, and the run
    // ended with no file and a record saying there was one.
    let taken = taken_away(
        stages
            .iter()
            .flat_map(PlanStage::changes)
            .map(Change::address),
        state,
        schema,
    );
    // ENFORCES: INV-PLAN-010
    // And what the removals that cannot be planned yet will take away once they
    // can. A declared resource naming one of those waits with it. Planned in the
    // meantime, it was adopted from the reading, and the removal, planned in
    // whichever later run its machine answered, took the thing away from under
    // that record.
    let withheld = taken_away(&held, state, schema);
    let mut outdated = Vec::new();
    // Marked as the walk goes. Stages are topological, so a resource's
    // dependencies have already been decided by the time it is reached, and
    // unreachability travels forward along the edges in a single pass.
    let mut unplannable: BTreeSet<Address> = BTreeSet::new();
    // What this plan is changing, so that a resource reacting to one of them
    // knows. Filled stage by stage, and a stage is only reached after every
    // stage its members depend on.
    let mut changing: BTreeSet<Address> = BTreeSet::new();

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
            let blocked = dependencies.get(address).is_some_and(|of_this| {
                of_this
                    .iter()
                    .any(|dependency| unplannable.contains(*dependency))
            });
            let name = name_of(resource, schema);
            let waits_for_removal = name.as_ref().is_some_and(|name| withheld.contains(name));

            if out_of_reach || blocked || waits_for_removal {
                unplannable.insert(address.clone());
                if let Some(node) = node {
                    let age = known_for(state, &node, now);
                    unreachable.push(Unreachable::new(address.clone(), node, age));
                }
                continue;
            }

            // A reaction's trigger is always in an earlier stage, because
            // reacting to something implies coming after it — so by the time
            // this resource is planned, whether its cause is changing is known.
            let roused_by: Vec<Address> = resource
                .reacts_to()
                .iter()
                .filter(|cause| changing.contains(*cause))
                .cloned()
                .collect();

            // A reaction an earlier run began and did not finish is owed, and
            // stays owed until it is paid. See decision 0026.
            let record = state.get(address);
            let owed: Vec<Address> = record
                .map(|record| record.owed().iter().cloned().collect())
                .unwrap_or_default();

            if let Some(change) = change_for(
                resource,
                observed,
                schema,
                secrets,
                &mut outdated,
                roused_by,
                owed,
                dependencies
                    .get(address)
                    .map(|of_this| {
                        of_this
                            .iter()
                            .map(|dependency| (*dependency).clone())
                            .collect()
                    })
                    .unwrap_or_default(),
                record.map(Record::depends_on),
                name.is_some_and(|name| taken.contains(&name)),
            )? {
                // ENFORCES: INV-REACT-001
                // Only what moves on the machine rouses anything. An adoption or
                // a reorder writes state and leaves the file as it was, and
                // counting one restarted every service reacting to a file that a
                // first run over configured machines adopted.
                if change.kind().sends_anything() {
                    changing.insert(address.clone());
                }
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

// ENFORCES: INV-PLAN-008
/// Refuses two resources on one machine that name the same thing there.
///
/// Addresses are unique by construction; what they declare is not. Two files at
/// one path were both created, the last write won, and every later plan proposed
/// the other again. Checked on declared values, so a name that arrives through a
/// reference is known only later and is not caught here.
fn one_thing_once(graph: &Graph, schema: &Schema) -> Result<(), PlanError> {
    let mut seen: BTreeMap<(Target, String, String), Address> = BTreeMap::new();
    for stage in graph.stages()? {
        for address in stage.addresses() {
            let (Some(node), Some(resource), Some((attribute, namespace))) = (
                address.target(),
                graph.get(address),
                schema.identity_of(address),
            ) else {
                continue;
            };
            let Some(Value::Text(value)) = resource.attribute(attribute).and_then(Attribute::known)
            else {
                continue;
            };
            let key = (node.clone(), namespace.to_owned(), one_spelling(value));
            if let Some(first) = seen.get(&key) {
                return Err(PlanError::SameThing {
                    first: first.clone(),
                    second: address.clone(),
                    node: node.clone(),
                    namespace: namespace.to_owned(),
                    value: value.clone(),
                });
            }
            seen.insert(key, address.clone());
        }
    }
    Ok(())
}

/// What removing each of `removed` takes away, named as `name_of` names a
/// declared resource: by machine, namespace and one spelling of the value.
///
/// Taken from state, because the configuration that named them is gone. A record
/// whose naming attribute is not a plain value names nothing here.
fn taken_away<'a>(
    removed: impl IntoIterator<Item = &'a Address>,
    state: &State,
    schema: &Schema,
) -> BTreeSet<(Target, String, String)> {
    removed
        .into_iter()
        .filter_map(|address| {
            let node = address.target()?;
            let (attribute, namespace) = schema.identity_of(address)?;
            let Value::Text(value) = state.get(address)?.attributes().get(attribute)?.plain()?
            else {
                return None;
            };
            Some((node.clone(), namespace.to_owned(), one_spelling(value)))
        })
        .collect()
}

/// What a declared resource names on its machine, if its provider says and the
/// value is known when planning.
fn name_of(resource: &Resource, schema: &Schema) -> Option<(Target, String, String)> {
    let address = resource.address();
    let node = address.target()?;
    let (attribute, namespace) = schema.identity_of(address)?;
    let Some(Value::Text(value)) = resource.attribute(attribute).and_then(Attribute::known) else {
        return None;
    };
    Some((node.clone(), namespace.to_owned(), one_spelling(value)))
}

// ENFORCES: INV-REACT-005
/// Refuses a reaction on a kind whose provider says it cannot react.
///
/// Refused where it is written as well, when a configuration is evaluated. This
/// is for a graph that did not come from one.
fn reactions_allowed(graph: &Graph, schema: &Schema) -> Result<(), PlanError> {
    for stage in graph.stages()? {
        for address in stage.addresses() {
            if let Some(resource) = graph.get(address)
                && !resource.reacts_to().is_empty()
                && !schema.reacts(address)
            {
                return Err(PlanError::CannotReact {
                    address: address.clone(),
                    kind: address.kind().as_str().to_owned(),
                });
            }
        }
    }
    Ok(())
}

/// A path without the spellings that name the same one: repeated separators, `.`
/// segments and a trailing separator. `..` is left as written, because through a
/// symbolic link it does not mean what it looks like.
fn one_spelling(path: &str) -> String {
    let kept: Vec<&str> = path
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect();
    let joined = kept.join("/");
    if path.starts_with('/') {
        format!("/{joined}")
    } else {
        joined
    }
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
///
/// Returned with the removals that cannot be planned yet, so that a declared
/// resource naming what one of them takes away can wait as well.
fn removals(
    graph: &Graph,
    state: &State,
    observed: &Observed,
    now: Written,
    unreachable: &mut Vec<Unreachable>,
) -> (Vec<PlanStage>, BTreeSet<Address>) {
    let declared: BTreeSet<&Address> = state
        .addresses()
        .filter(|address| graph.get(address).is_some())
        .collect();

    // ENFORCES: INV-REACH-001
    // A removal is planned from state rather than from a reading, so whether its
    // machine answered is asked here. One that did not is reported and not
    // planned, and so is every removal that has to wait for it: what a resource
    // comes after is removed after it. Planned anyway, it sat in a plan that
    // called itself complete, and apply stopped at the first stage, before
    // touching the machines that did answer.
    let mut held: BTreeSet<Address> = BTreeSet::new();
    let stages = state
        .abandoned(&declared)
        .into_iter()
        .filter_map(|level| {
            let changes: Vec<Change> = level
                .into_iter()
                .filter_map(|address| {
                    let node = address.target().cloned();
                    let out_of_reach = node.as_ref().is_some_and(|node| !observed.reached(node));
                    let waiting = held.iter().any(|earlier| {
                        state
                            .get(earlier)
                            .is_some_and(|record| record.depends_on().contains(&address))
                    });
                    if out_of_reach || waiting {
                        if let Some(node) = node {
                            let age = known_for(state, &node, now);
                            unreachable.push(Unreachable::new(address.clone(), node, age));
                        }
                        held.insert(address);
                        return None;
                    }
                    // Nothing was looked at: this resource is no longer
                    // declared, so the plan reasoned from state rather than
                    // from a reading, and there is nothing to have moved.
                    Some(Change::new(
                        address,
                        ChangeKind::Delete,
                        Vec::new(),
                        Vec::new(),
                        Vec::new(),
                        Vec::new(),
                        Vec::new(),
                        Vec::new(),
                    ))
                })
                .collect();
            (!changes.is_empty()).then(|| PlanStage::new(StageKind::Mixed, changes))
        })
        .collect();
    (stages, held)
}

/// What has to happen to one resource, or nothing if it already matches and is
/// recorded.
#[allow(
    clippy::too_many_arguments,
    reason = "what a change is decided from, one argument each, so none can be left out"
)]
fn change_for(
    resource: &Resource,
    observed: &Observed,
    schema: &Schema,
    secrets: Secrets<'_>,
    outdated: &mut Vec<Outdated>,
    roused_by: Vec<Address>,
    owed: Vec<Address>,
    depends_on: Vec<Address>,
    recorded: Option<&BTreeSet<Address>>,
    takes_over: bool,
) -> Result<Option<Change>, PlanError> {
    let address = resource.address();
    // Not read when it names what a removal in this plan takes away: see
    // `taken_away`.
    let existing = if takes_over {
        None
    } else {
        observed.get(address)
    };

    let mut deltas = Vec::new();
    let mut desired = Vec::new();
    for (name, declared) in resource.attributes() {
        let after = resolve(declared, observed, secrets)?;

        // ENFORCES: INV-PLAN-007
        // Refused while planning, so nothing is done on the strength of a plan
        // whose batch cannot be written. A secret is sealed before this point and
        // is checked by its provider before sending instead.
        if let Confidence::Known(value) = &after
            && let Some(most) = schema.bound(address, name)
            && let Some(bytes) = value.as_bytes()
            && bytes.len() > most
        {
            return Err(PlanError::TooLarge {
                address: address.clone(),
                attribute: name.clone(),
                size: bytes.len(),
                most,
            });
        }
        desired.push((name.clone(), after.clone()));

        // Reduced here, at the edge: a secret attribute's observed value is a
        // real credential read back off a machine, and this is the last place it
        // is allowed to be one. Everything downstream sees a digest.
        // ENFORCES: INV-SECRET-002
        let before = existing
            .and_then(|attributes| attributes.get(name))
            .map(|value| {
                if declared.is_secret() {
                    Recorded::Secret(secrets.digest(value))
                } else {
                    Recorded::Plain(value.clone())
                }
            });
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

    // The kind first, in the order it is decided — whether anything differs, then
    // why a resource that matches might still be work — and then the one change.
    let kind = match existing {
        // Not there at all: it has to be made, whatever its attributes say.
        None => ChangeKind::Create,
        Some(_) if deltas.iter().any(AttributeDelta::forces_replacement) => ChangeKind::Replace,
        Some(_) if !deltas.is_empty() => ChangeKind::Update,
        // It matches its declaration, and either something it reacts to does not
        // or an earlier run left a reaction unpaid. The desired values go with it
        // because the provider still needs to be told which service this is.
        Some(_) if !(roused_by.is_empty() && owed.is_empty()) => ChangeKind::React,
        // ENFORCES: INV-STATE-006
        // It matches its declaration and nothing records it: found on a machine
        // somebody configured before, or made by a run that stopped before it
        // could write that down. Nothing is done to it, and it is written down,
        // because from here on it is Nerpa's to remove. Planned as nothing, it
        // was never recorded, and taking its declaration away removed nothing.
        Some(_) if recorded.is_none() => ChangeKind::Adopt,
        // ENFORCES: INV-STATE-002
        // It matches and is recorded, and what it comes after is not what state
        // remembers: an `after` written in, a reference added, a reaction. Nothing
        // is done to it, and it is recorded again, because those edges are what
        // its removal will be ordered by. Planned as nothing, a file given an
        // `after` once it matched was removed with its directory in any order.
        Some(_) if recorded.is_some_and(|edges| !edges.iter().eq(&depends_on)) => {
            ChangeKind::Reorder
        }
        Some(_) => return Ok(None),
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

    Ok(Some(Change::new(
        address.clone(),
        kind,
        deltas,
        desired,
        premise,
        roused_by,
        owed,
        depends_on,
    )))
}

// ENFORCES: INV-PREMISE-002
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

// ENFORCES: INV-PLAN-003
/// A declared attribute, with references resolved against what already exists.
fn resolve(
    declared: &Attribute,
    observed: &Observed,
    secrets: Secrets<'_>,
) -> Result<Confidence, PlanError> {
    Ok(match declared {
        Attribute::Known(value) => Confidence::Known(value.clone()),
        Attribute::Unresolved(reference) => {
            match observed.attribute(reference.source(), reference.attribute()) {
                Some(value) => Confidence::Known(value.clone()),
                None => Confidence::Unknown(reference.clone()),
            }
        }
        // Fetched to be hashed, and dropped in `seal`. The only way to say
        // whether a secret changed is to look at what is there now.
        Attribute::Secret(source) => Confidence::Secret(secrets.seal(source)?),
        // Every branch is written down, so the graph is known whichever way this
        // goes. What is open is the value, and only within the set — which the
        // plan can print, and a person can approve.
        // ENFORCES: INV-FACT-001
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
