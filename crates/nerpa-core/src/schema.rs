//! What a difference in an attribute means.
//!
//! Three answers, not two, and the third is the one every other tool in this
//! space is missing.
//!
//! Most attributes are **converged**: they differ, so they are corrected. Some
//! are **immutable** — a machine's zone cannot move — so correcting them means
//! replacing the resource. And some describe how a resource was **born** rather
//! than what it must remain: the image it was made from, the user data it was
//! handed. Those are never corrected, because a running machine's boot disk
//! cannot be swapped underneath it and pretending otherwise turns an edit to an
//! image reference into a proposal to destroy the fleet.
//!
//! A birth attribute is not an ignored one. The difference is reported on every
//! plan, so that a fleet running on something no longer declared is a fact
//! somebody is told rather than one they find out. See
//! [0021](../../../docs/decisions).
//!
//! Which attribute is which is the provider's knowledge, not the model's. Until
//! providers exist this carries it as data, in the shape their schemas will
//! fill — `terraform providers schema -json` calls the second kind `ForceNew`,
//! and there is no reason to invent a different word for it.

use std::collections::{BTreeMap, BTreeSet};

use crate::address::{Address, Kind, Provider};

/// What each resource kind refuses to change in place.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Schema {
    immutable: BTreeMap<(Provider, Kind), BTreeSet<String>>,
    at_birth: BTreeMap<(Provider, Kind), BTreeSet<String>>,
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

    /// Declares that this attribute describes how the resource was made, not
    /// what it must remain.
    ///
    /// A difference is reported and never acted on. Opt-in and explicit, because
    /// getting it wrong this way means a change that should have happened never
    /// does — so silence has to be asked for.
    #[must_use]
    pub fn at_birth(
        mut self,
        provider: Provider,
        kind: Kind,
        attribute: impl Into<String>,
    ) -> Self {
        self.at_birth
            .entry((provider, kind))
            .or_default()
            .insert(attribute.into());
        self
    }

    /// Whether this attribute describes the resource's making rather than its
    /// state.
    #[must_use]
    pub fn is_at_birth(&self, address: &Address, attribute: &str) -> bool {
        self.at_birth
            .get(&(address.provider().clone(), address.kind().clone()))
            .is_some_and(|names| names.contains(attribute))
    }

    /// Folds another schema into this one.
    ///
    /// Providers each describe their own kinds, and a run needs them together.
    #[must_use]
    pub fn merge(mut self, other: Self) -> Self {
        for (kind, attributes) in other.immutable {
            self.immutable.entry(kind).or_default().extend(attributes);
        }
        for (kind, attributes) in other.at_birth {
            self.at_birth.entry(kind).or_default().extend(attributes);
        }
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
