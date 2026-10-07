//! One pass over the graph, deciding what each declared resource needs.

use std::collections::{BTreeMap, BTreeSet};

use crate::address::Address;
use crate::plan::{Confidence, Unreachable, Waiting};
use crate::resource::Resource;
use crate::state::Record;

use super::change::{Bearings, change_for};
use super::naming::{known_for, name_of, recorded_name};
use super::{Held, PlanError, Planning, Walk};

impl Planning<'_> {
    /// One walk of the graph, in the order the work will happen, knowing what the
    /// walks before it found held.
    #[dacc_derive::doc_anchor(id = "planner-walk")]
    #[allow(
        clippy::too_many_lines,
        reason = "one pass over the graph; splitting the per-resource step out would scatter what one pass must keep together"
    )]
    pub(super) fn walk(&self, held: &Held) -> Result<Walk, PlanError> {
        let mut walk = Walk::default();
        // What each already-planned resource will be, so that a reference to an
        // attribute changed in this same plan resolves to the new value rather
        // than the old one.
        let mut desired: BTreeMap<Address, BTreeMap<String, Confidence>> = BTreeMap::new();
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

                // Out of reach, or downstream of something that is. A resource
                // whose dependency could not be read has nowhere to take its
                // values from, so nothing can be said about it either.
                let node = address.domain().target().cloned();
                let out_of_reach = node
                    .as_ref()
                    .is_some_and(|node| !self.observed.reached(node));
                // A run ended while a change here was in flight. What that change
                // did is not known and cannot be found out, so nothing is planned
                // on this machine until a person says it is theirs again.
                let is_held = self.holds.contains_key(&node);
                let blocked = self.dependencies.get(address).is_some_and(|of_this| {
                    of_this
                        .iter()
                        .any(|dependency| walk.unplannable.contains(*dependency))
                });

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
                    .map_or(&[], Vec::as_slice);
                for removal in claimed {
                    walk.claims.insert(removal.clone(), address.clone());
                }
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

                if out_of_reach || is_held || blocked || waits_for_removal || waits.is_some() {
                    walk.unplannable.insert(address.clone());
                    self.owe_or_hold(&mut walk, resource, &changing);
                    // A held machine answered; it is simply not this run's to
                    // act on. Reported as one that did not answer, it would send
                    // somebody looking at the network.
                    if is_held {
                        *walk.holding.entry(node).or_default() += 1;
                        continue;
                    }
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
                let record = self.state.get(self.moves.get(address).unwrap_or(address));
                let owed: Vec<Address> =
                    record.map_or_else(Vec::new, |record| record.owed().iter().cloned().collect());

                if let Some(change) = change_for(
                    resource,
                    self.observed,
                    &desired,
                    self.schema,
                    self.secrets,
                    &mut walk.outdated,
                    &mut walk.merges,
                    Bearings {
                        roused_by,
                        owed,
                        depends_on: self.dependencies.get(address).map_or_else(
                            Vec::new,
                            |of_this| {
                                of_this
                                    .iter()
                                    .map(|dependency| (*dependency).clone())
                                    .collect()
                            },
                        ),
                        recorded: record.map(Record::depends_on),
                        entry: self
                            .state
                            .marked(self.moves.get(address).unwrap_or(address)),
                        takes_over: reads_what_is_taken,
                    },
                )? {
                    // Only what moves on the machine rouses anything. An adoption
                    // or a reorder writes state and leaves the file as it was, and
                    // counting one restarted every service reacting to a file that
                    // a first run over configured machines adopted.
                    if change.kind().sends_anything() {
                        changing.insert(address.clone());
                    }
                    desired.insert(address.clone(), change.desired().iter().cloned().collect());
                    walk.changes.push(change);
                }
            }
        }
        Ok(walk)
    }

    #[dacc_derive::doc_anchor(id = "inv-react-006-1")]
    /// What this plan sends that rouses `resource`, which cannot be planned.
    ///
    /// The reaction is owed on its record, written before the cause is sent and
    /// paid in whichever later run it can be planned. With no record there is
    /// nowhere to owe it, so the cause waits. Sent anyway, the file changed, the
    /// service on a machine that did not answer kept reading the old one, and when
    /// the machine came back nothing was changing and nothing was owed.
    pub(super) fn owe_or_hold(
        &self,
        walk: &mut Walk,
        resource: &Resource,
        changing: &BTreeSet<Address>,
    ) {
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

    #[dacc_derive::doc_anchor(id = "inv-reach-004-2")]
    /// What a resource that cannot be planned, and is not out of reach itself,
    /// waits for.
    ///
    /// Said to wait, and for what. Listed as out of reach, a change held on a
    /// machine that answered was reported as that machine not answering, and one on
    /// no machine was not reported at all.
    pub(super) fn waits_on(
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
}
