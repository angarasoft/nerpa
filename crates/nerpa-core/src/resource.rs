//! One node of the desired-state graph.
//!
//! A resource carries what it should look like, what it must come after, and how
//! Nerpa is allowed to treat it when the machine disagrees. Nothing here knows
//! how to read or change anything: that is the provider's job. This is the shape
//! the planner compares against reality.

use std::collections::{BTreeMap, BTreeSet};

use crate::address::Address;
use crate::attribute::Attribute;
use crate::requires;

/// What Nerpa may do when reality has drifted from the declaration.
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
#[dacc_derive::doc_anchor(id = "inv-drift-001")]
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
    attributes: BTreeMap<String, Attribute>,
    depends_on: BTreeSet<Address>,
    reacts_to: BTreeSet<Address>,
    protection: Protection,
}

impl Resource {
    /// A resource with no attributes and no declared dependencies.
    pub fn new(address: Address) -> Self {
        Self {
            address,
            attributes: BTreeMap::new(),
            depends_on: BTreeSet::new(),
            reacts_to: BTreeSet::new(),
            protection: Protection::default(),
        }
    }

    /// Sets one attribute, replacing any previous value.
    ///
    /// Takes a settled value or a [`Reference`](crate::attribute::Reference) to
    /// one that does not exist yet. A reference also becomes an edge in the
    /// graph, so nobody has to declare that ordering a second time.
    ///
    /// # Panics
    ///
    /// Panics if `name` is empty, or if the attribute reads from this resource's
    /// own address. See [`crate::contract`].
    #[must_use]
    pub fn with(mut self, name: impl Into<String>, value: impl Into<Attribute>) -> Self {
        let name = name.into();
        let value = value.into();
        requires!(
            !name.is_empty(),
            "attribute of {} has an empty name",
            self.address
        );
        requires!(
            value
                .reference()
                .is_none_or(|reference| reference.source() != &self.address),
            "attribute {name} of {} reads from itself",
            self.address
        );
        self.attributes.insert(name, value);
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

    /// Declares that this resource must act again when `other` changes.
    ///
    /// An edge, not a name in a global table: a typo is an evaluation error with
    /// a line number, two modules cannot collide over "restart nginx", and the
    /// restart is ordered after the file rather than piled at the end of a run.
    /// Ordering follows, because acting before the thing that caused it would be
    /// acting on the old world. See decision 0026.
    ///
    /// # Panics
    ///
    /// Panics if `other` is this resource's own address. See [`crate::contract`].
    #[must_use]
    pub fn reacting_to(mut self, other: Address) -> Self {
        requires!(
            other != self.address,
            "{} was declared to react to itself",
            self.address
        );
        self.depends_on.insert(other.clone());
        self.reacts_to.insert(other);
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
    pub fn attributes(&self) -> &BTreeMap<String, Attribute> {
        &self.attributes
    }

    /// One declared attribute, if it was set.
    pub fn attribute(&self, name: &str) -> Option<&Attribute> {
        self.attributes.get(name)
    }

    /// What this resource must be applied after.
    pub fn depends_on(&self) -> &BTreeSet<Address> {
        &self.depends_on
    }

    /// What this resource must act again for, when that changes.
    pub fn reacts_to(&self) -> &BTreeSet<Address> {
        &self.reacts_to
    }

    /// What Nerpa may do when the machine disagrees with the declaration.
    pub fn protection(&self) -> Protection {
        self.protection
    }
}
