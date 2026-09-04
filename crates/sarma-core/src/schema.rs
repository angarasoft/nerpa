//! Which attributes cannot be changed in place.
//!
//! Whether an attribute can be altered on a living resource is the provider's
//! knowledge, not the model's: a virtual machine's zone cannot move, its label
//! can. Until providers exist this carries the same information as data, in the
//! shape their schemas will fill — `terraform providers schema -json` calls it
//! `ForceNew`, and there is no reason to invent a different word for it.

use std::collections::{BTreeMap, BTreeSet};

use crate::address::{Address, Kind, Provider};

/// What each resource kind refuses to change in place.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Schema {
    immutable: BTreeMap<(Provider, Kind), BTreeSet<String>>,
}

impl Schema {
    /// A schema that lets every attribute be changed in place.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Declares that changing `attribute` on this kind means replacing it.
    #[must_use]
    pub fn immutable(
        mut self,
        provider: Provider,
        kind: Kind,
        attribute: impl Into<String>,
    ) -> Self {
        self.immutable
            .entry((provider, kind))
            .or_default()
            .insert(attribute.into());
        self
    }

    /// Whether changing this attribute of this resource forces a replacement.
    #[must_use]
    pub fn forces_replacement(&self, address: &Address, attribute: &str) -> bool {
        self.immutable
            .get(&(address.provider().clone(), address.kind().clone()))
            .is_some_and(|names| names.contains(attribute))
    }
}
