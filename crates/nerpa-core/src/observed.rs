//! What the world looks like, as far as anything has been read.
//!
//! Deliberately a plain record rather than a live view. Everything is read
//! before planning starts ([0006](../../../docs/decisions)), so by the time the
//! planner runs there is nothing left to ask anybody — which is what lets a plan
//! be complete before the first action.
//!
//! Absence means the resource does not exist. It does not mean "not read yet":
//! a target that could not be reached is a refusal to plan, not an empty answer,
//! because guessing it is empty would propose creating things that are already
//! there.

use std::collections::{BTreeMap, BTreeSet};

use crate::address::{Address, Target};
use crate::value::Value;

/// Observed state, gathered before planning.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Observed {
    resources: BTreeMap<Address, BTreeMap<String, Value>>,
    unreachable: BTreeSet<Target>,
}

impl Observed {
    /// Nothing read, and therefore nothing existing.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records what one resource currently looks like.
    pub fn record(&mut self, address: Address, attributes: BTreeMap<String, Value>) {
        self.resources.insert(address, attributes);
    }

    /// What a resource currently looks like, or `None` if it does not exist.
    #[must_use]
    pub fn get(&self, address: &Address) -> Option<&BTreeMap<String, Value>> {
        self.resources.get(address)
    }

    /// One observed attribute, if the resource exists and carries it.
    #[must_use]
    pub fn attribute(&self, address: &Address, name: &str) -> Option<&Value> {
        self.resources.get(address)?.get(name)
    }

    /// Whether a resource exists at all.
    #[must_use]
    pub fn contains(&self, address: &Address) -> bool {
        self.resources.contains_key(address)
    }

    /// Every address that was found to exist, in a stable order.
    pub fn addresses(&self) -> impl Iterator<Item = &Address> {
        self.resources.keys()
    }

    /// How many resources were found.
    #[must_use]
    pub fn len(&self) -> usize {
        self.resources.len()
    }

    /// Whether nothing was found.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.resources.is_empty()
    }

    /// Records that a machine did not answer.
    ///
    /// Distinct from finding nothing on it. Nothing found means the resources
    /// are not there; not answering means we do not know, and the difference
    /// decides whether a plan may say anything about them at all.
    pub fn unreachable(&mut self, node: Target) {
        self.unreachable.insert(node);
    }

    /// Whether this machine answered.
    #[must_use]
    pub fn reached(&self, node: &Target) -> bool {
        !self.unreachable.contains(node)
    }

    /// Every machine that did not answer, in a stable order.
    #[must_use]
    pub fn unreachable_nodes(&self) -> &BTreeSet<Target> {
        &self.unreachable
    }
}
