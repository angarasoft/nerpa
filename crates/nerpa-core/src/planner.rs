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

mod change;
mod naming;
mod removals;
mod walk;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::address::{Address, Target};
use crate::graph::{Graph, GraphError, Moved, Stage};
use crate::observed::Observed;
use crate::plan::{Change, Merge, Outdated, Plan, Unreachable, Waiting};
use crate::schema::Schema;
use crate::secret::Secrets;
use crate::state::{State, Written};

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
    Secret(#[from] crate::secret::FetchError),
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
        "{address}: {attribute} is {value:?}, and a mode is written as text, in octal within the permission bits, such as \"0644\"; a mode written as a number cannot be told from a decimal once the configuration is evaluated, so it is quoted"
    )]
    NotAMode {
        /// The resource declaring it.
        address: Address,
        /// Which attribute.
        attribute: String,
        /// What it says.
        value: String,
    },
    /// A monotonic attribute declared smaller than it is.
    ///
    /// A disk can be enlarged but not shrunk. The cloud would refuse the
    /// change anyway; the plan refuses it first, by name, rather than
    /// proposing a change that cannot be made.
    #[error(
        "{address}: {attribute} is declared as {will}, and {was} is what is there: a value that only grows cannot shrink"
    )]
    ShrinkRefused {
        /// The resource declaring it.
        address: Address,
        /// Which attribute.
        attribute: String,
        /// What is there.
        was: i64,
        /// What is declared.
        will: i64,
    },
    /// Changes that each have to come after the next, round to the first. One run
    /// cannot order them.
    #[error("one run cannot order these, each after the next: {}", steps(.cycle))]
    Unorderable {
        /// Each step round the cycle.
        cycle: Vec<Step>,
    },
    /// A rename that would change the provider, the kind or the slot of a
    /// resource. A rename migrates the address, not what the resource is; see
    /// 0028.
    #[error(
        "{from} cannot be renamed to {to}: a rename cannot change the provider, the kind or the slot"
    )]
    Move {
        /// Where it was declared.
        from: Address,
        /// Where it is declared now.
        to: Address,
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
    /// A resource made after what named the same thing moves away from it.
    MadeAfterMove,
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
            Because::MadeAfterMove => write!(
                f,
                "making {later} after {earlier} moves away from what it names"
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

/// The renames this plan applies, and where each new address finds its record.
///
/// A rename is a declared migration (decision 0028): the record at `from` is
/// the record for `to`, so `to` is planned against what was recorded at `from`
/// and `from` is not removed. A migration already recorded in state is spent
/// and is not carried again. A rename across a provider, a kind or a slot is
/// refused.
#[dacc_derive::doc_anchor(id = "inv-plan-013")]
#[allow(
    clippy::type_complexity,
    reason = "a rename migrates a record to its new address and reports the spent ones; the three collections are one decision"
)]
fn declared_migrations(
    graph: &Graph,
    state: &State,
) -> Result<(BTreeMap<Address, Address>, Vec<Moved>, Vec<Moved>), PlanError> {
    let mut moves: BTreeMap<Address, Address> = BTreeMap::new();
    let mut migrations: Vec<Moved> = Vec::new();
    let mut spent: Vec<Moved> = Vec::new();
    for moved in graph.migrations() {
        if moved.from().provider() != moved.to().provider()
            || moved.from().kind() != moved.to().kind()
            || moved.from().slot() != moved.to().slot()
        {
            return Err(PlanError::Move {
                from: moved.from().clone(),
                to: moved.to().clone(),
            });
        }
        if state.migrations().contains(moved) {
            spent.push(moved.clone());
        } else {
            moves.insert(moved.to().clone(), moved.from().clone());
            migrations.push(moved.clone());
        }
    }
    Ok((moves, migrations, spent))
}

/// Computes what would have to happen for the world to match the configuration.
///
/// # Errors
///
/// Returns [`GraphError`] if the configuration cannot be ordered at all — a
/// cycle, a dangling dependency, or a resource on a node the inventory does not
/// know. Nothing is planned for a graph that cannot be walked.
#[dacc_derive::doc_anchor(id = "inv-plan-002")]
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
    naming::one_thing_once(graph, schema)?;
    naming::reactions_allowed(graph, schema)?;
    let planning = Planning::new(graph, observed, state, schema, secrets, now)?;

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
            .chain(&walk.unplannable)
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
        // A move that cannot be planned holds what names the thing it leaves.
        held.removals.extend(
            walk.unplannable
                .iter()
                .filter(|address| walk.claims.contains_key(*address))
                .cloned(),
        );
        held.changes.extend(walk.hold);
        held.changes.extend(replacing);
    }
}

/// What a walk is told is held, by the walks before it.
#[derive(Default)]
struct Held {
    /// Removals, and moves away from a name, that cannot be planned.
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
    /// A rename migrates the record: the new address's record lives at the old
    /// one until the migration is applied. Keyed by the new address.
    moves: BTreeMap<Address, Address>,
    /// The renames this plan applies, so apply can move the records and record
    /// them. Already-applied renames are not here.
    migrations: Vec<Moved>,
    /// The renames this state has already applied, which the plan reports can
    /// be removed from the configuration.
    spent: Vec<Moved>,
    /// What state remembers and the configuration no longer declares, in the
    /// order it can be removed in.
    doomed: Vec<Vec<Address>>,
    /// What each of those takes away, by what names it on its machine.
    taken: BTreeMap<(Target, String, String), Vec<Address>>,
    /// What state recorded as coming after each address.
    recorded_after: BTreeMap<&'a Address, Vec<&'a Address>>,
    /// The domains a run left mid-change, and when the oldest change still in
    /// flight there was written. Nothing on one of these is planned.
    holds: BTreeMap<Option<Target>, Written>,
}

impl<'a> Planning<'a> {
    /// Everything one plan is computed from, worked out before the graph is
    /// walked.
    ///
    /// # Errors
    ///
    /// The same as [`plan_at`]: the graph cannot be ordered, or a rename would
    /// change what a resource is.
    fn new(
        graph: &'a Graph,
        observed: &'a Observed,
        state: &'a State,
        schema: &'a Schema,
        secrets: Secrets<'a>,
        now: Written,
    ) -> Result<Self, PlanError> {
        // Once, not once per resource. Asking the graph for a single resource's
        // dependencies rebuilds every edge it has, and this loop asks for every
        // resource.
        let dependencies = graph.dependencies()?;
        // A rename is a declared migration: the record at `from` is the record
        // for `to`, and `from` is not removed. See 0028.
        let (moves, migrations, spent) = declared_migrations(graph, state)?;
        let migrating: BTreeSet<Address> = moves.values().cloned().collect();

        // Everything state remembers that the configuration still asks for —
        // records and entries alike, because that is what `abandoned` decides
        // over. Records alone, and an address remembered only by an entry was
        // removed and made again in the same plan.
        let declared: BTreeSet<&Address> = state
            .remembered()
            .filter(|address| graph.get(address).is_some())
            .collect();
        let doomed: Vec<Vec<Address>> = state
            .abandoned(&declared)
            .into_iter()
            .map(|level| {
                level
                    .into_iter()
                    .filter(|address| !migrating.contains(address))
                    .collect()
            })
            .filter(|level: &Vec<Address>| !level.is_empty())
            .collect();
        Ok(Self {
            graph,
            observed,
            state,
            schema,
            secrets,
            now,
            dependencies,
            stages: graph.stages()?,
            taken: naming::taken_away(&doomed, graph, state, schema),
            recorded_after: removals::recorded_after(state),
            doomed,
            holds: holding(state),
            moves,
            migrations,
            spent,
        })
    }
}

/// The domains a run left mid-change, and when the oldest change still in flight
/// there was written.
///
/// A mark in flight is a request written down as going out and never answered:
/// whether it arrived, and what it did, went with the run that stopped. So the
/// domain is held, and this is what says since when. RFC 0001 §5.8.
fn holding(state: &State) -> BTreeMap<Option<Target>, Written> {
    let mut holds: BTreeMap<Option<Target>, Written> = BTreeMap::new();
    for (address, mark) in state.marks() {
        if !mark.in_flight() {
            continue;
        }
        holds
            .entry(address.target().cloned())
            .and_modify(|since| {
                if mark.written().seconds() < since.seconds() {
                    *since = mark.written();
                }
            })
            .or_insert_with(|| mark.written());
    }
    holds
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
    merges: Vec<Merge>,
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
    /// How many of this walk's resources each held domain holds.
    holding: BTreeMap<Option<Target>, usize>,
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
    /// How many removals each held domain holds.
    holding: BTreeMap<Option<Target>, usize>,
}
