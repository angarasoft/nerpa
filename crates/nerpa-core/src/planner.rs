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

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use crate::address::{Address, Target};
use crate::attribute::Attribute;
use crate::graph::{Graph, GraphError, Stage, StageKind};
use crate::observed::Observed;
use crate::plan::{
    AttributeDelta, Change, ChangeKind, Confidence, Outdated, Plan, PlanStage, Unreachable, Waiting,
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
    /// A file mode written as something no file can have.
    #[error(
        "{address}: {attribute} is {value:?}, and a mode is written in octal within the permission bits, such as \"0644\""
    )]
    NotAMode {
        /// The resource declaring it.
        address: Address,
        /// Which attribute.
        attribute: String,
        /// What it says.
        value: String,
    },
    /// Changes that each have to come after the next, round to the first. One run
    /// cannot order them.
    #[error("one run cannot order these, each after the next: {}", steps(.cycle))]
    Unorderable {
        /// Each step round the cycle.
        cycle: Vec<Step>,
    },
}

/// One step round an order a run cannot follow: `later` has to come after
/// `earlier`, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// What has to come after.
    pub later: Address,
    /// What it has to come after.
    pub earlier: Address,
    /// Why.
    pub because: Because,
}

/// Why one thing in a plan has to come after another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Because {
    /// It is declared after it, reads from it, or reacts to it.
    Declared,
    /// A removal after the removal of what state recorded after it.
    RemovedAfterRemoval,
    /// A removal after the change of what state recorded after it.
    RemovedAfterChange,
    /// A resource made after the removal of what names the same thing.
    MadeAfterRemoval,
    /// A replacement after the removal of what state recorded after it.
    ReplacedAfterRemoval,
    /// A replacement after the change of what state recorded after it.
    ReplacedAfterChange,
}

impl fmt::Display for Step {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (later, earlier) = (&self.later, &self.earlier);
        match self.because {
            Because::Declared => write!(f, "{later} after {earlier}, as declared"),
            Because::RemovedAfterRemoval => write!(
                f,
                "removing {later} after removing {earlier}, which state recorded after it"
            ),
            Because::RemovedAfterChange => write!(
                f,
                "removing {later} after changing {earlier}, which state recorded after it"
            ),
            Because::MadeAfterRemoval => write!(
                f,
                "making {later} after removing {earlier}, which takes away what it names"
            ),
            Because::ReplacedAfterRemoval => write!(
                f,
                "replacing {later} after removing {earlier}, which state recorded after it"
            ),
            Because::ReplacedAfterChange => write!(
                f,
                "replacing {later} after changing {earlier}, which state recorded after it"
            ),
        }
    }
}

/// The steps of a refusal, and what a person can do about it.
fn steps(cycle: &[Step]) -> String {
    let joined = cycle
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ");
    format!("{joined}. Change one of them in a run of its own first")
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

    let declared: BTreeSet<&Address> = state
        .addresses()
        .filter(|address| graph.get(address).is_some())
        .collect();
    let doomed = state.abandoned(&declared);
    let planning = Planning {
        graph,
        observed,
        state,
        schema,
        secrets,
        now,
        stages: graph.stages()?,
        taken: taken_away(&doomed, state, schema),
        recorded_after: recorded_after(state),
        doomed,
        dependencies,
    };

    // Holding goes both ways. A removal waits while something recorded after it
    // cannot be planned, and a declared resource waits while the removal of what
    // it names does. A cause waits while a resource reacting to it cannot be
    // planned and has no record to owe the reaction on, and a replacement waits
    // while what was recorded after it cannot be planned. So the walk is taken
    // again when it finds something held that it did not know was. Once in the
    // ordinary case, and at most once more for each thing held that way. It is
    // still 0006's single pass in what that decision is about: one plan, made in
    // full from one reading before anything runs, and no planning between stages.
    let mut held = Held::default();
    loop {
        let walk = planning.walk(&held)?;
        let removals = planning.removals(&walk);
        let replacing = planning.waiting_replacements(&walk, &removals);
        let named = removals
            .held
            .iter()
            .any(|removal| !held.removals.contains(removal) && walk.claims.contains_key(removal));
        let waiting = walk
            .hold
            .keys()
            .chain(replacing.keys())
            .any(|change| !held.changes.contains_key(change));
        if !named && !waiting {
            return planning.assemble(walk, removals);
        }
        held.removals.extend(removals.held);
        held.changes.extend(walk.hold);
        held.changes.extend(replacing);
    }
}

/// What a walk is told is held, by the walks before it.
#[derive(Default)]
struct Held {
    /// Removals that cannot be planned.
    removals: BTreeSet<Address>,
    /// Declared changes that wait, and what for: a cause rousing a resource that
    /// cannot be planned and has no record, or a replacement whose removal of what
    /// was there waits for something that cannot be planned.
    changes: BTreeMap<Address, Address>,
}

/// What a plan is computed from, and what is worked out from that before the
/// graph is walked.
struct Planning<'a> {
    graph: &'a Graph,
    observed: &'a Observed,
    state: &'a State,
    schema: &'a Schema,
    secrets: Secrets<'a>,
    now: Written,
    dependencies: BTreeMap<&'a Address, BTreeSet<&'a Address>>,
    stages: Vec<Stage>,
    /// What state remembers and the configuration no longer declares, in the
    /// order it can be removed in.
    doomed: Vec<Vec<Address>>,
    /// What each of those takes away, by what names it on its machine.
    taken: BTreeMap<(Target, String, String), Vec<Address>>,
    /// What state recorded as coming after each address.
    recorded_after: BTreeMap<&'a Address, Vec<&'a Address>>,
}

/// What one walk of the graph decided, before the removals are put among it.
#[derive(Default)]
struct Walk {
    /// The declared changes, in the order of the walk.
    changes: Vec<Change>,
    /// The declared resources nothing could be said about.
    unplannable: BTreeSet<Address>,
    unreachable: Vec<Unreachable>,
    outdated: Vec<Outdated>,
    /// For each removal, the declared resource naming what it takes away.
    claims: BTreeMap<Address, Address>,
    /// For each cause, the resources it rouses that cannot be planned and have a
    /// record to owe the reaction on.
    owed_by: BTreeMap<Address, Vec<Address>>,
    /// Causes rousing a resource that cannot be planned and has no record, and
    /// that resource.
    hold: BTreeMap<Address, Address>,
    /// What waits for something that cannot be planned, and what for.
    waiting: Vec<Waiting>,
}

/// The removals one walk leaves: which this plan carries, and which it cannot.
#[derive(Default)]
struct Removals {
    /// The removals this plan carries, in reverse dependency order.
    planned: Vec<Change>,
    /// Which of state's levels of removal each of those is in.
    level: BTreeMap<Address, usize>,
    /// How many levels those take: the first rank anything declared can have, so
    /// that what can be removed first is.
    levels: usize,
    /// What cannot be removed in this plan.
    held: BTreeSet<Address>,
    unreachable: Vec<Unreachable>,
    waiting: Vec<Waiting>,
}

impl Planning<'_> {
    /// One walk of the graph, in the order the work will happen, knowing what the
    /// walks before it found held.
    fn walk(&self, held: &Held) -> Result<Walk, PlanError> {
        let mut walk = Walk::default();
        // What this plan is changing, so that a resource reacting to one of them
        // knows. Filled stage by stage, and a stage is only reached after every
        // stage its members depend on.
        let mut changing: BTreeSet<Address> = BTreeSet::new();

        // The graph's stages are topological, so a resource's dependencies have
        // been decided by the time it is reached, and unreachability travels
        // forward along the edges as the walk goes. Where each change goes is
        // decided afterwards, with the removals.
        for stage in &self.stages {
            for address in stage.addresses() {
                let Some(resource) = self.graph.get(address) else {
                    continue;
                };

                // ENFORCES: INV-REACH-001
                // Out of reach, or downstream of something that is. A resource
                // whose dependency could not be read has nowhere to take its
                // values from, so nothing can be said about it either.
                let node = address.domain().target().cloned();
                let out_of_reach = node
                    .as_ref()
                    .is_some_and(|node| !self.observed.reached(node));
                let blocked = self.dependencies.get(address).is_some_and(|of_this| {
                    of_this
                        .iter()
                        .any(|dependency| walk.unplannable.contains(*dependency))
                });

                // ENFORCES: INV-PLAN-009
                // What a removal takes away, by what names it on its machine. A
                // declared resource naming the same thing is made again after the
                // removal rather than read from before it: the reading describes a
                // thing this plan is about to delete. Renaming an address and
                // keeping its path, with no `moved`, used to remove the file and
                // adopt the new resource from that reading, and the run ended with
                // no file and a record saying there was one.
                let name = name_of(resource, self.schema);
                let claimed: &[Address] = name
                    .as_ref()
                    .and_then(|name| self.taken.get(name))
                    .map(Vec::as_slice)
                    .unwrap_or_default();
                for removal in claimed {
                    walk.claims.insert(removal.clone(), address.clone());
                }
                // ENFORCES: INV-PLAN-010
                // And one naming what a removal that cannot be planned yet will
                // take away once it can waits with it. Planned in the meantime, it
                // was adopted from the reading, and the removal, planned in
                // whichever later run its machine answered, took the thing away
                // from under that record.
                let waits_for_removal = claimed
                    .iter()
                    .any(|removal| held.removals.contains(removal));
                // Its reading describes the thing a removal takes away only when
                // it was taken where the resource is declared. One that state
                // recorded somewhere else was read there, and is the resource's
                // own: it is replaced after the removal, which removes the old one.
                // Planned as a creation, it was made at the new path, and the old
                // path stayed on the machine with nothing recording it.
                let reads_what_is_taken = !claimed.is_empty()
                    && recorded_name(address, self.state, self.schema)
                        .is_none_or(|recorded| Some(&recorded) == name.as_ref());
                // A cause rousing something that cannot be planned and has
                // nowhere to owe the reaction, or a replacement waiting for what
                // was recorded after it, found by a walk before this one.
                let waits = held.changes.get(address);

                if out_of_reach || blocked || waits_for_removal || waits.is_some() {
                    walk.unplannable.insert(address.clone());
                    self.owe_or_hold(&mut walk, resource, &changing);
                    let on = if out_of_reach {
                        None
                    } else {
                        self.waits_on(&walk, address, claimed, held)
                    };
                    match (on, node) {
                        (Some(on), _) => walk.waiting.push(Waiting::new(address.clone(), on)),
                        (None, Some(node)) => {
                            let age = known_for(self.state, &node, self.now);
                            walk.unreachable
                                .push(Unreachable::new(address.clone(), node, age));
                        }
                        (None, None) => {}
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
                let record = self.state.get(address);
                let owed: Vec<Address> = record
                    .map(|record| record.owed().iter().cloned().collect())
                    .unwrap_or_default();

                if let Some(change) = change_for(
                    resource,
                    self.observed,
                    self.schema,
                    self.secrets,
                    &mut walk.outdated,
                    roused_by,
                    owed,
                    self.dependencies
                        .get(address)
                        .map(|of_this| {
                            of_this
                                .iter()
                                .map(|dependency| (*dependency).clone())
                                .collect()
                        })
                        .unwrap_or_default(),
                    record.map(Record::depends_on),
                    reads_what_is_taken,
                )? {
                    // ENFORCES: INV-REACT-001
                    // Only what moves on the machine rouses anything. An adoption
                    // or a reorder writes state and leaves the file as it was, and
                    // counting one restarted every service reacting to a file that
                    // a first run over configured machines adopted.
                    if change.kind().sends_anything() {
                        changing.insert(address.clone());
                    }
                    walk.changes.push(change);
                }
            }
        }
        Ok(walk)
    }

    // ENFORCES: INV-REACT-006
    /// What this plan sends that rouses `resource`, which cannot be planned.
    ///
    /// The reaction is owed on its record, written before the cause is sent and
    /// paid in whichever later run it can be planned. With no record there is
    /// nowhere to owe it, so the cause waits. Sent anyway, the file changed, the
    /// service on a machine that did not answer kept reading the old one, and when
    /// the machine came back nothing was changing and nothing was owed.
    fn owe_or_hold(&self, walk: &mut Walk, resource: &Resource, changing: &BTreeSet<Address>) {
        let address = resource.address();
        let record = self.state.get(address);
        for cause in resource
            .reacts_to()
            .iter()
            .filter(|cause| changing.contains(*cause))
        {
            if record.is_some() {
                walk.owed_by
                    .entry(cause.clone())
                    .or_default()
                    .push(address.clone());
            } else {
                walk.hold.insert(cause.clone(), address.clone());
            }
        }
    }

    // ENFORCES: INV-REACH-004
    /// What a resource that cannot be planned, and is not out of reach itself,
    /// waits for.
    ///
    /// Said to wait, and for what. Listed as out of reach, a change held on a
    /// machine that answered was reported as that machine not answering, and one on
    /// no machine was not reported at all.
    fn waits_on(
        &self,
        walk: &Walk,
        address: &Address,
        claimed: &[Address],
        held: &Held,
    ) -> Option<Address> {
        self.dependencies
            .get(address)
            .and_then(|of_this| {
                of_this
                    .iter()
                    .find(|dependency| walk.unplannable.contains(**dependency))
            })
            .map(|dependency| (*dependency).clone())
            .or_else(|| {
                claimed
                    .iter()
                    .find(|removal| held.removals.contains(*removal))
                    .cloned()
            })
            .or_else(|| held.changes.get(address).cloned())
    }

    /// Which removals this plan carries, and which it cannot.
    ///
    /// In reverse dependency order, taken from state because the configuration
    /// that described a dependency is exactly what was deleted. Where each goes is
    /// decided with everything else, in `assemble`.
    fn removals(&self, walk: &Walk) -> Removals {
        let mut removals = Removals::default();
        for (index, level) in self.doomed.iter().enumerate() {
            let before = removals.planned.len();
            for address in level {
                // ENFORCES: INV-REACH-001
                // A removal is planned from state rather than from a reading, so
                // whether its machine answered is asked here. One that did not is
                // reported and not planned, and so is every removal that has to
                // wait for it: what a resource comes after is removed after it.
                // Planned anyway, it sat in a plan that called itself complete,
                // and apply stopped at the first stage, before touching the
                // machines that did answer. The same for a removal waiting on a
                // declared resource nothing can be said about, which may still be
                // in the way.
                let node = address.target().cloned();
                let out_of_reach = node
                    .as_ref()
                    .is_some_and(|node| !self.observed.reached(node));
                let waiting = self.removal_waits_for(walk, &removals, address);
                if out_of_reach || waiting.is_some() {
                    // ENFORCES: INV-REACH-004
                    // Out of reach only when its own machine did not answer.
                    match (out_of_reach, waiting, node) {
                        (false, Some(on), _) => {
                            removals.waiting.push(Waiting::new(address.clone(), on));
                        }
                        (_, _, Some(node)) => {
                            let age = known_for(self.state, &node, self.now);
                            removals
                                .unreachable
                                .push(Unreachable::new(address.clone(), node, age));
                        }
                        _ => {}
                    }
                    removals.held.insert(address.clone());
                    continue;
                }

                // Nothing was looked at: this resource is no longer declared, so
                // the plan reasoned from state rather than from a reading, and
                // there is nothing to have moved.
                removals.level.insert(address.clone(), index);
                removals.planned.push(Change::new(
                    address.clone(),
                    ChangeKind::Delete,
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                ));
            }
            if removals.planned.len() > before {
                removals.levels += 1;
            }
        }
        removals
    }

    /// Replacements whose removal of what was there has to wait for something
    /// that cannot be planned, and what for: what state recorded after them,
    /// declared and nothing said about it, or a removal that is held.
    fn waiting_replacements(&self, walk: &Walk, removals: &Removals) -> BTreeMap<Address, Address> {
        walk.changes
            .iter()
            .filter(|change| change.kind() == ChangeKind::Replace)
            .filter_map(|change| {
                self.later(change.address())
                    .iter()
                    .find(|other| {
                        **other != change.address()
                            && (removals.held.contains(**other)
                                || (walk.unplannable.contains(**other)
                                    && self.may_move_out(other, change.address())))
                    })
                    .map(|other| (change.address().clone(), (*other).clone()))
            })
            .collect()
    }

    /// The plan: every change and removal in one order that keeps every edge
    /// between them, or a refusal naming an order no run can follow.
    ///
    /// Each gets a rank above the ranks of everything it has to come after, and
    /// the stages are the ranks. Removals start at the first rank, and what is
    /// declared after the removals state can take first, so a removal that waits
    /// for nothing still comes before everything declared: a resource on its way
    /// out can be holding something a new one needs — a name, a port, a path.
    fn assemble(&self, walk: Walk, removals: Removals) -> Result<Plan, PlanError> {
        let steps = self.edges(&walk, &removals);

        // Everything declared takes part, changing or not, so an order the graph
        // gives through something that does not change is kept.
        let mut nodes: BTreeMap<Address, usize> = BTreeMap::new();
        for stage in &self.stages {
            for address in stage.addresses() {
                nodes.insert(address.clone(), removals.levels);
            }
        }
        for removal in &removals.planned {
            nodes.insert(removal.address().clone(), 0);
        }
        let rank = ranked(&nodes, &steps)?;

        // ENFORCES: INV-REACT-006
        // Each cause carries the debts it leaves on records this plan cannot
        // reach, so that apply writes them before sending it.
        let mut declared: BTreeMap<Address, Change> = walk
            .changes
            .into_iter()
            .map(|change| (change.address().clone(), change))
            .collect();
        for (cause, reactors) in walk.owed_by {
            if let Some(change) = declared.get_mut(&cause) {
                change.leave_owed(reactors);
            }
        }

        let mut leaving: BTreeMap<usize, Vec<Change>> = BTreeMap::new();
        for removal in removals.planned {
            let at = rank.get(removal.address()).copied().unwrap_or_default();
            leaving.entry(at).or_default().push(removal);
        }
        // In the graph's order within a rank, with a rank's kind taken from
        // everything declared in it, as the graph takes a stage's.
        let mut arriving: BTreeMap<usize, (Vec<Address>, Vec<Change>)> = BTreeMap::new();
        for stage in &self.stages {
            for address in stage.addresses() {
                let at = rank.get(address).copied().unwrap_or_default();
                let (all, changes) = arriving.entry(at).or_default();
                all.push(address.clone());
                if let Some(change) = declared.remove(address) {
                    changes.push(change);
                }
            }
        }

        let mut stages = Vec::new();
        let ranks: BTreeSet<usize> = leaving.keys().chain(arriving.keys()).copied().collect();
        for at in ranks {
            // What leaves before what arrives at the same rank.
            if let Some(changes) = leaving.remove(&at) {
                stages.push(PlanStage::new(StageKind::Mixed, changes));
            }
            // A rank where everything already matches is not a stage anybody
            // needs to read about.
            if let Some((all, changes)) = arriving.remove(&at)
                && !changes.is_empty()
            {
                stages.push(PlanStage::new(StageKind::of(&all), changes));
            }
        }

        let mut unreachable = removals.unreachable;
        unreachable.extend(walk.unreachable);
        let mut waiting = removals.waiting;
        waiting.extend(walk.waiting);
        Ok(Plan::new(stages, unreachable, waiting, walk.outdated))
    }

    /// Every edge between one plan's changes and removals, and why each is there.
    fn edges(&self, walk: &Walk, removals: &Removals) -> BTreeMap<(Address, Address), Because> {
        let removing: BTreeSet<&Address> = removals.planned.iter().map(Change::address).collect();
        let sending: BTreeMap<&Address, ChangeKind> = walk
            .changes
            .iter()
            .filter(|change| change.kind().sends_anything())
            .map(|change| (change.address(), change.kind()))
            .collect();

        let mut steps: BTreeMap<(Address, Address), Because> = BTreeMap::new();
        let mut step = |later: &Address, earlier: &Address, because: Because| {
            steps
                .entry((later.clone(), earlier.clone()))
                .or_insert(because);
        };
        for (dependent, of) in &self.dependencies {
            for dependency in of {
                step(dependent, dependency, Because::Declared);
            }
        }
        for address in removing.iter().copied() {
            for other in self.later(address).iter().copied() {
                // ENFORCES: INV-STATE-002
                // What a resource came after is removed after it. Not within a
                // level state could not order, which it removes together.
                let earlier_level = removals
                    .level
                    .get(other)
                    .is_some_and(|level| removals.level.get(address) > Some(level));
                if removing.contains(other) && earlier_level {
                    step(address, other, Because::RemovedAfterRemoval);
                }
                // ENFORCES: INV-PLAN-011
                // After the change of whatever state recorded after it. A file
                // recorded after its directory and moved out of it while the
                // directory's declaration went was still inside when the directory
                // was removed; the removal refused, and every later run did the
                // same.
                if self.moves_out(&sending, other, address) {
                    step(address, other, Because::RemovedAfterChange);
                }
            }
            // ENFORCES: INV-PLAN-009
            // What names the thing a removal takes away is made after it.
            if let Some(taker) = walk.claims.get(address) {
                step(taker, address, Because::MadeAfterRemoval);
            }
        }
        for (address, kind) in &sending {
            if *kind != ChangeKind::Replace {
                continue;
            }
            for other in self.later(address).iter().copied() {
                if other == *address {
                    continue;
                }
                // ENFORCES: INV-PLAN-011
                // A replacement's removal of what was there likewise. Removals
                // first had kept a directory replaced after the removal of a file
                // recorded inside it without the planner saying so, and ordering
                // that removal after something else moved it past the replacement:
                // the directory was deleted with the file still in it.
                if removing.contains(other) {
                    step(address, other, Because::ReplacedAfterRemoval);
                } else if self.moves_out(&sending, other, address) {
                    step(address, other, Because::ReplacedAfterChange);
                }
            }
        }
        steps
    }

    /// What a removal waits for, if anything: a removal state recorded after it
    /// that is held, or a declared resource that can move out of it and cannot be
    /// planned.
    fn removal_waits_for(
        &self,
        walk: &Walk,
        removals: &Removals,
        address: &Address,
    ) -> Option<Address> {
        self.later(address)
            .iter()
            .copied()
            .find(|other| {
                removals.held.contains(*other)
                    || (walk.unplannable.contains(*other) && self.may_move_out(other, address))
            })
            .cloned()
    }

    /// Whether the change planned for `later` has to come before `earlier`'s
    /// removal of what is there: it sends something that can move it, and it can
    /// move out of `earlier`.
    fn moves_out(
        &self,
        sending: &BTreeMap<&Address, ChangeKind>,
        later: &Address,
        earlier: &Address,
    ) -> bool {
        matches!(
            sending.get(later),
            Some(ChangeKind::Update | ChangeKind::Replace)
        ) && self.may_move_out(later, earlier)
    }

    // ENFORCES: INV-PLAN-011
    /// Whether `later`, declared and recorded after `earlier`, can be in the way of
    /// removing what `earlier` is: on the same machine, of a kind whose provider
    /// names what it is there, and not declared under the name state recorded.
    ///
    /// Only then does its change have to come first, or hold the removal while it
    /// cannot be planned. A reaction's record names its cause without being inside
    /// it, and a resource on another machine is inside nothing here: ordering
    /// either first refused a moved file together with the service reacting to it,
    /// put a removal nothing was inside after an update, and held a replacement
    /// whose reaction could have been owed.
    fn may_move_out(&self, later: &Address, earlier: &Address) -> bool {
        if later.target() != earlier.target() || self.schema.identity_of(later).is_none() {
            return false;
        }
        let declared = self
            .graph
            .get(later)
            .and_then(|resource| name_of(resource, self.schema));
        let recorded = recorded_name(later, self.state, self.schema);
        declared.is_none() || recorded.is_none() || declared != recorded
    }

    /// What state recorded as coming after `address`.
    fn later(&self, address: &Address) -> &[&Address] {
        self.recorded_after
            .get(address)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }
}

/// Each address's rank: its own, raised above the rank of everything it has to
/// come after. Refused, naming a cycle, when there is no such order.
fn ranked(
    nodes: &BTreeMap<Address, usize>,
    steps: &BTreeMap<(Address, Address), Because>,
) -> Result<BTreeMap<Address, usize>, PlanError> {
    let mut waiting: BTreeMap<&Address, usize> = nodes.keys().map(|address| (address, 0)).collect();
    let mut after: BTreeMap<&Address, Vec<&Address>> = BTreeMap::new();
    for (later, earlier) in steps.keys() {
        if nodes.contains_key(earlier)
            && let Some(count) = waiting.get_mut(later)
        {
            *count += 1;
            after.entry(earlier).or_default().push(later);
        }
    }

    let mut rank: BTreeMap<&Address, usize> =
        nodes.iter().map(|(address, own)| (address, *own)).collect();
    let mut ready: VecDeque<&Address> = waiting
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(address, _)| *address)
        .collect();
    let mut placed = 0usize;
    while let Some(earlier) = ready.pop_front() {
        placed += 1;
        let at = rank.get(earlier).copied().unwrap_or_default();
        for later in after.get(earlier).into_iter().flatten().copied() {
            if let Some(own) = rank.get_mut(later) {
                *own = (*own).max(at + 1);
            }
            if let Some(count) = waiting.get_mut(later) {
                *count -= 1;
                if *count == 0 {
                    ready.push_back(later);
                }
            }
        }
    }

    if placed < nodes.len() {
        return Err(PlanError::Unorderable {
            cycle: cycle(&waiting, steps),
        });
    }
    Ok(rank
        .into_iter()
        .map(|(address, at)| (address.clone(), at))
        .collect())
}

/// One cycle among what could not be placed, as the steps round it.
///
/// Everything still waiting waits for something else still waiting, so walking
/// back from any of them comes round again.
fn cycle(
    waiting: &BTreeMap<&Address, usize>,
    steps: &BTreeMap<(Address, Address), Because>,
) -> Vec<Step> {
    let stuck: BTreeSet<&Address> = waiting
        .iter()
        .filter(|(_, count)| **count > 0)
        .map(|(address, _)| *address)
        .collect();
    let Some(mut at) = stuck.iter().next().copied() else {
        return Vec::new();
    };
    let mut path: Vec<Step> = Vec::new();
    let mut seen: Vec<&Address> = Vec::new();
    loop {
        if let Some(start) = seen.iter().position(|visited| *visited == at) {
            return path.split_off(start);
        }
        seen.push(at);
        let Some(((later, earlier), because)) = steps
            .iter()
            .find(|((later, earlier), _)| later == at && stuck.contains(earlier))
        else {
            return path;
        };
        path.push(Step {
            later: later.clone(),
            earlier: earlier.clone(),
            because: *because,
        });
        at = earlier;
    }
}

/// What state recorded as coming after each address: each record's edges, the
/// other way round.
fn recorded_after(state: &State) -> BTreeMap<&Address, Vec<&Address>> {
    let mut after: BTreeMap<&Address, Vec<&Address>> = BTreeMap::new();
    for address in state.addresses() {
        if let Some(record) = state.get(address) {
            for dependency in record.depends_on() {
                after.entry(dependency).or_default().push(address);
            }
        }
    }
    after
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

/// What each removal takes away, named as `name_of` names a declared resource: by
/// machine, namespace and one spelling of the value.
///
/// Taken from state, because the configuration that named them is gone. A record
/// whose naming attribute is not a plain value names nothing here.
fn taken_away(
    doomed: &[Vec<Address>],
    state: &State,
    schema: &Schema,
) -> BTreeMap<(Target, String, String), Vec<Address>> {
    let mut taken: BTreeMap<(Target, String, String), Vec<Address>> = BTreeMap::new();
    for address in doomed.iter().flatten() {
        if let Some(name) = recorded_name(address, state, schema) {
            taken.entry(name).or_default().push(address.clone());
        }
    }
    taken
}

/// What a record names on its machine, if its provider says and state kept the
/// value.
fn recorded_name(
    address: &Address,
    state: &State,
    schema: &Schema,
) -> Option<(Target, String, String)> {
    let node = address.target()?;
    let (attribute, namespace) = schema.identity_of(address)?;
    let Value::Text(value) = state.get(address)?.attributes().get(attribute)?.plain()? else {
        return None;
    };
    Some((node.clone(), namespace.to_owned(), one_spelling(value)))
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
        // ENFORCES: INV-PLAN-012
        // A mode is the number it spells. Compared as written, "644" never matched
        // the "0644" a reading reports, and every run changed it again; one beyond
        // the permission bits never matched at all, because a machine keeps only
        // those. So it is compared as a number, and what spells no mode a file can
        // have is refused here, before anything is done on the strength of it.
        let mode = schema.is_mode(address, name);
        if mode
            && let Confidence::Known(Value::Text(written)) = &after
            && crate::schema::mode_of(written).is_none()
        {
            return Err(PlanError::NotAMode {
                address: address.clone(),
                attribute: name.clone(),
                value: written.clone(),
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
/// Whether a mode on the machine is the mode declared, as numbers.
fn same_mode(before: Option<&Recorded>, after: &Confidence) -> bool {
    match (before, after) {
        (Some(Recorded::Plain(Value::Text(current))), Confidence::Known(Value::Text(wanted))) => {
            crate::schema::mode_of(current).is_some()
                && crate::schema::mode_of(current) == crate::schema::mode_of(wanted)
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
