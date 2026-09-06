//! Where the nodes come from.
//!
//! A target-domain address names its node as text — `os:package.nginx@web-01` —
//! and nothing in that text says who brings `web-01` into existence. The
//! inventory says it, so that everything on a node can be ordered after whatever
//! creates it.
//!
//! Machines Nerpa does not manage are declared here too, as
//! [`Provenance::Preexisting`]. A fleet part-way through adoption is then not a
//! special case: both kinds of node are said out loud, in the same place.

use std::collections::BTreeMap;

use crate::address::{Address, Target};

/// What brings a node into existence.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum Provenance {
    /// A resource in this graph creates it, and everything on the node waits for
    /// that resource.
    Managed(Address),
    /// It is already there, and Nerpa does not create or destroy it.
    Preexisting,
}

/// Which nodes exist, and where each one comes from.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Inventory {
    nodes: BTreeMap<Target, Provenance>,
}

impl Inventory {
    /// An inventory that knows of no nodes at all.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Declares that `resource` brings `target` into existence.
    ///
    /// Replaces any previous declaration for that node.
    pub fn managed_by(&mut self, target: Target, resource: Address) {
        self.nodes.insert(target, Provenance::Managed(resource));
    }

    /// Declares that `target` is already there and Nerpa does not create it.
    ///
    /// Replaces any previous declaration for that node.
    pub fn preexisting(&mut self, target: Target) {
        self.nodes.insert(target, Provenance::Preexisting);
    }

    /// Every node this inventory knows, in a stable order.
    ///
    /// Stable because a run that reaches machines in an order nobody can predict
    /// is a run nobody can compare against the last one.
    pub fn nodes(&self) -> impl Iterator<Item = &Target> {
        self.nodes.keys()
    }

    /// Where a node comes from, if the inventory has been told.
    #[must_use]
    pub fn provenance(&self, target: &Target) -> Option<&Provenance> {
        self.nodes.get(target)
    }

    /// How many nodes are declared.
    #[must_use]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether nothing is declared.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}
