//! One node of the desired-state graph.
//!
//! A resource carries what it should look like, what it must come after, and how
//! Sarma is allowed to treat it when the machine disagrees. Nothing here knows
//! how to read or change anything: that is the provider's job. This is the shape
//! the planner compares against reality.

use std::collections::{BTreeMap, BTreeSet};

use crate::address::Address;
use crate::requires;
use crate::value::Value;

/// What Sarma may do when reality has drifted from the declaration.
///
/// The default is what every other tool does unconditionally, and the second
/// option is the one they are missing. Portage has had it for twenty-five years
/// as `CONFIG_PROTECT`: a configuration file somebody edited by hand is never
/// silently replaced, the new version is staged beside it and a person merges
/// the two. Terraform's answer to drift is to overwrite; Ansible's is to
/// overwrite. Both are a claim about operational reality that is simply untrue —
/// people do edit machines during an incident, and the edit is usually the only
/// record of what the incident was.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[non_exhaustive]
// ENFORCES: INV-DRIFT-001
pub enum Protection {
    /// Drift is corrected: the declaration wins.
    #[default]
    Converge,
    /// Drift stops the stage and is handed to a person as a three-way merge.
    Merge,
}

/// A resource in the desired state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resource {
    address: Address,
    attributes: BTreeMap<String, Value>,
    depends_on: BTreeSet<Address>,
    protection: Protection,
}

impl Resource {
    /// A resource with no attributes and no declared dependencies.
    pub fn new(address: Address) -> Self {
        Self {
            address,
            attributes: BTreeMap::new(),
            depends_on: BTreeSet::new(),
            protection: Protection::default(),
        }
    }

    /// Sets one attribute, replacing any previous value.
    ///
    /// # Panics
    ///
    /// Panics if `name` is empty. See [`crate::contract`].
    #[must_use]
    pub fn with(mut self, name: impl Into<String>, value: impl Into<Value>) -> Self {
        let name = name.into();
        requires!(
            !name.is_empty(),
            "attribute of {} has an empty name",
            self.address
        );
        self.attributes.insert(name, value.into());
        self
    }

    /// Declares that this resource must be applied after `other`.
    ///
    /// Dependencies that follow from an attribute referring to another
    /// resource's output are discovered, not declared; this is for the ordering
    /// that the values alone do not reveal.
    ///
    /// # Panics
    ///
    /// Panics if `other` is this resource's own address. See [`crate::contract`].
    #[must_use]
    pub fn after(mut self, other: Address) -> Self {
        requires!(
            other != self.address,
            "{} was declared to depend on itself",
            self.address
        );
        self.depends_on.insert(other);
        self
    }

    /// Marks the resource as one that must not be silently overwritten.
    #[must_use]
    pub fn merge_on_drift(mut self) -> Self {
        self.protection = Protection::Merge;
        self
    }

    /// Where this resource lives.
    pub fn address(&self) -> &Address {
        &self.address
    }

    /// Everything declared about it, by name.
    pub fn attributes(&self) -> &BTreeMap<String, Value> {
        &self.attributes
    }

    /// One declared attribute, if it was set.
    pub fn attribute(&self, name: &str) -> Option<&Value> {
        self.attributes.get(name)
    }

    /// What this resource must be applied after.
    pub fn depends_on(&self) -> &BTreeSet<Address> {
        &self.depends_on
    }

    /// What Sarma may do when the machine disagrees with the declaration.
    pub fn protection(&self) -> Protection {
        self.protection
    }
}
