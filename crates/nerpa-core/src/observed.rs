//! What the world looks like, as far as anything has been read.
//!
//! Expands: 0011
//!
//! Deliberately a plain record rather than a live view. Everything is read
//! before planning starts (`0006`), so by the time the
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

/// What one machine turned out to be.
///
/// Handed to a provider so that it can decide how to ask — `dnf` or `emerge` is
/// something read from the machine, not something guessed from an address. The
/// engine gathers these once per machine in a journey of its own, so a provider
/// asking for itself would be a second journey that `INV-BATCH-001` exists to
/// prevent.
#[derive(Debug, Clone, Copy)]
pub struct Facts<'a> {
    said: &'a BTreeMap<String, String>,
}

impl Facts<'_> {
    /// What the machine said to one question, if it was asked and answered.
    #[must_use]
    pub fn said(&self, name: &str) -> Option<&str> {
        self.said.get(name).map(String::as_str)
    }

    /// Everything the machine said, for a provider that reads them all.
    #[must_use]
    pub fn all(&self) -> &BTreeMap<String, String> {
        self.said
    }

    /// Whether nothing is known about the machine at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.said.is_empty()
    }
}

/// Observed state, gathered before planning.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Observed {
    facts: BTreeMap<Target, BTreeMap<String, String>>,
    resources: BTreeMap<Address, BTreeMap<String, Value>>,
    /// What was looked at to decide what each resource is, and what it hashed
    /// to. Kept beside the values rather than inside them because it is not a
    /// value: nothing plans from it, and the only thing that reads it is the
    /// batch that goes back to the machine. See decision 0034.
    premise: BTreeMap<Address, Vec<nerpa_proto::Premise>>,
    unreachable: BTreeSet<Target>,
}

impl Observed {
    /// Records what a machine turned out to be.
    ///
    /// Facts are readings like any other, gathered in the same journey. They are
    /// kept apart from resource attributes because nothing created them and
    /// nothing depends on them existing — see decision 0029.
    pub fn learn(&mut self, node: Target, name: impl Into<String>, said: impl Into<String>) {
        self.facts
            .entry(node)
            .or_default()
            .insert(name.into(), said.into());
    }

    /// What a machine said about itself, if it was asked and answered.
    #[must_use]
    pub fn fact(&self, node: &Target, name: &str) -> Option<&str> {
        self.facts.get(node)?.get(name).map(String::as_str)
    }

    /// Nothing read, and therefore nothing existing.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// What a machine turned out to be, for handing to a provider.
    ///
    /// Empty for the control domain, which has no machine to have turned out
    /// to be anything.
    #[must_use]
    pub fn facts_of(&self, node: Option<&Target>) -> Facts<'_> {
        static NONE: std::sync::OnceLock<BTreeMap<String, String>> = std::sync::OnceLock::new();
        Facts {
            said: node
                .and_then(|node| self.facts.get(node))
                .unwrap_or_else(|| NONE.get_or_init(BTreeMap::new)),
        }
    }

    /// What was looked at to decide about a resource.
    pub fn looked_at(&mut self, address: Address, premise: Vec<nerpa_proto::Premise>) {
        if premise.is_empty() {
            return;
        }
        self.premise.insert(address, premise);
    }

    /// What was looked at to decide about a resource, or nothing if it was not
    /// there to look at.
    #[must_use]
    pub fn premise(&self, address: &Address) -> &[nerpa_proto::Premise] {
        self.premise.get(address).map_or(&[], Vec::as_slice)
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
