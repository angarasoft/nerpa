//! Ordering the removals, and turning every change and removal into one order
//! that keeps every edge between them.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::address::Address;
use crate::graph::StageKind;
use crate::plan::{Change, ChangeKind, Plan, PlanStage, Unreachable, Waiting};
use crate::state::State;

use super::naming::{known_for, name_of, recorded_name};
use super::{Because, PlanError, Planning, Removals, Step, Walk};

impl Planning<'_> {
    /// Which removals this plan carries, and which it cannot.
    ///
    /// In reverse dependency order, taken from state because the configuration
    /// that described a dependency is exactly what was deleted. Where each goes is
    /// decided with everything else, in `assemble`.
    #[dacc_derive::doc_anchor(id = "planner-removals")]
    pub(super) fn removals(&self, walk: &Walk) -> Removals {
        let mut removals = Removals::default();
        for (index, level) in self.doomed.iter().enumerate() {
            let before = removals.planned.len();
            for address in level {
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
                let is_held = self.holds.contains_key(&node);
                let waiting = self.removal_waits_for(walk, &removals, address);
                if out_of_reach || is_held || waiting.is_some() {
                    if is_held {
                        *removals.holding.entry(node).or_default() += 1;
                        removals.held.insert(address.clone());
                        continue;
                    }
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
    pub(super) fn waiting_replacements(
        &self,
        walk: &Walk,
        removals: &Removals,
    ) -> BTreeMap<Address, Address> {
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
    #[dacc_derive::doc_anchor(id = "inv-react-006-2")]
    pub(super) fn assemble(&self, walk: Walk, removals: Removals) -> Result<Plan, PlanError> {
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
            let at = rank.get(removal.address()).copied().unwrap_or(0);
            leaving.entry(at).or_default().push(removal);
        }
        // In the graph's order within a rank, with a rank's kind taken from
        // everything declared in it, as the graph takes a stage's.
        let mut arriving: BTreeMap<usize, (Vec<Address>, Vec<Change>)> = BTreeMap::new();
        for stage in &self.stages {
            for address in stage.addresses() {
                let at = rank.get(address).copied().unwrap_or(0);
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
        // Every domain a run left mid-change, whether or not this plan wanted
        // to do anything there: the machine is held either way, and somebody has
        // to be told before they can release it.
        let held: Vec<crate::plan::Held> = self
            .holds
            .iter()
            .map(|(domain, since)| {
                let changes = walk.holding.get(domain).copied().unwrap_or(0)
                    + removals.holding.get(domain).copied().unwrap_or(0);
                crate::plan::Held::new(domain.clone(), *since, changes)
            })
            .collect();
        let mut waiting = removals.waiting;
        waiting.extend(walk.waiting);
        Ok(Plan::new(
            stages,
            unreachable,
            waiting,
            held,
            walk.outdated,
            walk.merges,
            self.migrations.clone(),
            self.spent.clone(),
        ))
    }

    /// Every edge between one plan's changes and removals, and why each is there.
    #[dacc_derive::doc_anchor(id = "planner-edges")]
    pub(super) fn edges(
        &self,
        walk: &Walk,
        removals: &Removals,
    ) -> BTreeMap<(Address, Address), Because> {
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
                // What a resource came after is removed after it. Not within a
                // level state could not order, which it removes together.
                let earlier_level = removals
                    .level
                    .get(other)
                    .is_some_and(|level| removals.level.get(address) > Some(level));
                if removing.contains(other) && earlier_level {
                    step(address, other, Because::RemovedAfterRemoval);
                }
                // After the change of whatever state recorded after it. A file
                // recorded after its directory and moved out of it while the
                // directory's declaration went was still inside when the directory
                // was removed; the removal refused, and every later run did the
                // same.
                if self.moves_out(&sending, other, address) {
                    step(address, other, Because::RemovedAfterChange);
                }
            }
            // What names the thing a removal takes away is made after it.
            if let Some(taker) = walk.claims.get(address) {
                step(taker, address, Because::MadeAfterRemoval);
            }
        }
        // And what names the thing a declared resource moves away from is made
        // after the move.
        for (moving, taker) in &walk.claims {
            if !removing.contains(moving) && sending.contains_key(moving) {
                step(taker, moving, Because::MadeAfterMove);
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
    ///
    /// Named by the root of the wait, not by the first that comes to hand. One
    /// whose own wait leads back here is passed over, however many steps it takes
    /// to lead back, or the plan would say each waits for the other and never name
    /// the machine that did not answer.
    pub(super) fn removal_waits_for(
        &self,
        walk: &Walk,
        removals: &Removals,
        address: &Address,
    ) -> Option<Address> {
        let waits: BTreeMap<&Address, &Address> = walk
            .waiting
            .iter()
            .chain(&removals.waiting)
            .map(|entry| (entry.address(), entry.on()))
            .collect();
        let leads_back = |from: &Address| {
            let mut at = from;
            for _ in 0..=waits.len() {
                match waits.get(at) {
                    Some(next) if *next == address => return true,
                    Some(next) => at = next,
                    None => return false,
                }
            }
            true
        };
        let candidates: Vec<&Address> = self
            .later(address)
            .iter()
            .copied()
            .filter(|other| {
                removals.held.contains(*other)
                    || (walk.unplannable.contains(*other) && self.may_move_out(other, address))
            })
            .collect();
        candidates
            .iter()
            .copied()
            .find(|other| !leads_back(other))
            // Everything it waits for waits for it in turn, which takes a record of
            // an order no run made. Naming one of them says more than naming none.
            .or_else(|| candidates.first().copied())
            .cloned()
    }

    /// Whether the change planned for `later` has to come before `earlier`'s
    /// removal of what is there: it sends something that can move it, and it can
    /// move out of `earlier`.
    #[dacc_derive::doc_anchor(id = "inv-plan-011-3")]
    pub(super) fn moves_out(
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
    pub(super) fn may_move_out(&self, later: &Address, earlier: &Address) -> bool {
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
    pub(super) fn later(&self, address: &Address) -> &[&Address] {
        self.recorded_after.get(address).map_or(&[], Vec::as_slice)
    }
}

/// Each address's rank: its own, raised above the rank of everything it has to
/// come after. Refused, naming a cycle, when there is no such order.
pub(super) fn ranked(
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
        let at = rank.get(earlier).copied().unwrap_or(0);
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
pub(super) fn cycle(
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
#[dacc_derive::doc_anchor(id = "inv-plan-008")]
pub(super) fn recorded_after(state: &State) -> BTreeMap<&Address, Vec<&Address>> {
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
