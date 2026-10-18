//! What a difference in an attribute means.
//!
//! Expands: 0021
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
//! `0021`.
//!
//! Which attribute is which is the provider's knowledge, not the model's. Until
//! providers exist this carries it as data, in the shape their schemas will
//! fill — `terraform providers schema -json` calls the second kind `ForceNew`,
//! and there is no reason to invent a different word for it.

use std::collections::{BTreeMap, BTreeSet};

use crate::address::{Address, Kind, Provider};
use crate::value::Value;

/// What each resource kind refuses to change in place.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Schema {
    immutable: BTreeMap<(Provider, Kind), BTreeSet<String>>,
    at_birth: BTreeMap<(Provider, Kind), BTreeSet<String>>,
    bounded: BTreeMap<(Provider, Kind), BTreeMap<String, usize>>,
    identities: BTreeMap<(Provider, Kind), (String, String)>,
    unreactive: BTreeSet<(Provider, Kind)>,
    modes: BTreeMap<(Provider, Kind), BTreeSet<String>>,
    monotonic: BTreeMap<(Provider, Kind), BTreeSet<String>>,
    stop_to_change: BTreeMap<(Provider, Kind), BTreeSet<String>>,
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

    /// Declares that this attribute names the thing on the machine, within a
    /// namespace that several kinds can share.
    ///
    /// A file and a directory at one path are one thing on the machine however
    /// many addresses declare them, so both name their path in the namespace
    /// `path`. Two resources on one machine naming the same thing are refused,
    /// because otherwise each is written over the other on every run.
    #[must_use]
    pub fn identity(
        mut self,
        provider: Provider,
        kind: Kind,
        attribute: impl Into<String>,
        namespace: impl Into<String>,
    ) -> Self {
        self.identities
            .insert((provider, kind), (attribute.into(), namespace.into()));
        self
    }

    /// Which attribute names this resource's thing on its machine, and in which
    /// namespace, if its provider says.
    #[must_use]
    pub fn identity_of(&self, address: &Address) -> Option<(&str, &str)> {
        self.identities
            .get(&(address.provider().clone(), address.kind().clone()))
            .map(|(attribute, namespace)| (attribute.as_str(), namespace.as_str()))
    }

    /// Declares the most, in bytes, this attribute may carry.
    ///
    /// For a value that has to reach a machine in one operation. The planner
    /// refuses a known value over it, so a file too big to send is refused before
    /// anything happens rather than when its batch is written.
    #[must_use]
    pub fn bounded(
        mut self,
        provider: Provider,
        kind: Kind,
        attribute: impl Into<String>,
        most: usize,
    ) -> Self {
        self.bounded
            .entry((provider, kind))
            .or_default()
            .insert(attribute.into(), most);
        self
    }

    /// The most, in bytes, this attribute of this resource may carry, if there is
    /// a most.
    #[must_use]
    pub fn bound(&self, address: &Address, attribute: &str) -> Option<usize> {
        self.bounded
            .get(&(address.provider().clone(), address.kind().clone()))
            .and_then(|bounds| bounds.get(attribute).copied())
    }

    /// Declares that this attribute is a file mode, written in octal.
    ///
    /// Compared as the number it spells, so "644", "0644" and "0o644" are one
    /// mode, and refused while planning when it spells no mode a file can have.
    #[must_use]
    pub fn mode(mut self, provider: Provider, kind: Kind, attribute: impl Into<String>) -> Self {
        self.modes
            .entry((provider, kind))
            .or_default()
            .insert(attribute.into());
        self
    }

    /// Whether this attribute of this resource is a file mode.
    #[must_use]
    pub fn is_mode(&self, address: &Address, attribute: &str) -> bool {
        self.modes
            .get(&(address.provider().clone(), address.kind().clone()))
            .is_some_and(|names| names.contains(attribute))
    }

    /// Declares that this attribute may only grow, never shrink.
    ///
    /// A disk can be enlarged but not shrunk. A declared value below what is
    /// there is refused while planning rather than proposed as a change the
    /// cloud would refuse anyway; a value above it is a normal change.
    /// Compared as integers; a shrink of a value of another shape is the
    /// provider's to refuse.
    #[must_use]
    pub fn monotonic(
        mut self,
        provider: Provider,
        kind: Kind,
        attribute: impl Into<String>,
    ) -> Self {
        self.monotonic
            .entry((provider, kind))
            .or_default()
            .insert(attribute.into());
        self
    }

    /// Whether this attribute of this resource may only grow.
    #[must_use]
    pub fn grows_only(&self, address: &Address, attribute: &str) -> bool {
        self.monotonic
            .get(&(address.provider().clone(), address.kind().clone()))
            .is_some_and(|names| names.contains(attribute))
    }

    /// Declares that changing this attribute requires the resource to be
    /// stopped first.
    ///
    /// A running instance's cores and memory cannot be changed in place: the
    /// cloud refuses. The change is proposed with the stop it requires, so the
    /// provider stops, applies and starts rather than failing mid-run.
    #[must_use]
    pub fn stop_to_change(
        mut self,
        provider: Provider,
        kind: Kind,
        attribute: impl Into<String>,
    ) -> Self {
        self.stop_to_change
            .entry((provider, kind))
            .or_default()
            .insert(attribute.into());
        self
    }

    /// Whether changing this attribute of this resource requires a stop first.
    #[must_use]
    pub fn requires_stop(&self, address: &Address, attribute: &str) -> bool {
        self.stop_to_change
            .get(&(address.provider().clone(), address.kind().clone()))
            .is_some_and(|names| names.contains(attribute))
    }

    /// Declares that this kind cannot react to anything.
    ///
    /// A reaction is something a change elsewhere asks a resource to do again: a
    /// restart, for a service. A kind with nothing to do again says so, and a
    /// configuration giving it `reacts_to` is refused where it says it, rather
    /// than doing nothing on one family of machines and everything on another.
    #[must_use]
    pub fn cannot_react(mut self, provider: Provider, kind: Kind) -> Self {
        self.unreactive.insert((provider, kind));
        self
    }

    /// Whether this resource may react to anything.
    #[must_use]
    pub fn reacts(&self, address: &Address) -> bool {
        !self
            .unreactive
            .contains(&(address.provider().clone(), address.kind().clone()))
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
        // Two schemas bounding the same attribute keep the smaller bound: the
        // larger one would let through what the other says cannot be carried.
        for (kind, bounds) in other.bounded {
            let mine = self.bounded.entry(kind).or_default();
            for (attribute, most) in bounds {
                mine.entry(attribute)
                    .and_modify(|held| *held = (*held).min(most))
                    .or_insert(most);
            }
        }
        self.identities.extend(other.identities);
        self.unreactive.extend(other.unreactive);
        for (kind, attributes) in other.modes {
            self.modes.entry(kind).or_default().extend(attributes);
        }
        for (kind, attributes) in other.monotonic {
            self.monotonic.entry(kind).or_default().extend(attributes);
        }
        for (kind, attributes) in other.stop_to_change {
            self.stop_to_change
                .entry(kind)
                .or_default()
                .extend(attributes);
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

#[dacc_derive::doc_anchor(id = "inv-plan-012-2")]
/// The mode a file mode written in octal spells: `"0644"`, `"644"` or `"0o644"`.
///
/// `None` for text that is not octal, and for a number beyond the permission bits,
/// which a machine does not keep: the rest of a mode is the kind of file, and a
/// file written with it reads back without it.
#[must_use]
pub fn mode_of(written: &str) -> Option<u32> {
    let digits = written.strip_prefix("0o").unwrap_or(written);
    if digits.is_empty() || !digits.bytes().all(|digit| (b'0'..=b'7').contains(&digit)) {
        return None;
    }
    u32::from_str_radix(digits, 8)
        .ok()
        .filter(|mode| *mode <= 0o7777)
}

/// The mode a declared value spells, which is text and nothing else.
///
/// A number is refused rather than read. Starlark evaluates `0o600` to 384 before
/// any of this sees it, so a mode written as a number cannot be told from one
/// meant as a decimal: reading text as octal and a number as itself made `400` the
/// mode `0620`, group writable, where `"400"` is `0400`.
#[must_use]
pub fn mode_of_value(value: &Value) -> Option<u32> {
    match value {
        Value::Text(written) => mode_of(written),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::address::{Kind, Provider, address};

    #[test]
    fn monotonic_and_stop_to_change_are_asked_by_name() {
        let provider = Provider::new("yandex").expect("a valid provider name");
        let schema = Schema::new()
            .monotonic(
                provider.clone(),
                Kind::new("disk").expect("a valid kind"),
                "size",
            )
            .stop_to_change(
                provider,
                Kind::new("instance").expect("a valid kind"),
                "cores",
            );

        let disk = address("yandex:disk.data");
        let instance = address("yandex:instance.db");

        assert!(schema.grows_only(&disk, "size"));
        assert!(!schema.grows_only(&disk, "type"));
        assert!(schema.requires_stop(&instance, "cores"));
        assert!(!schema.requires_stop(&instance, "zone"));
    }

    #[test]
    fn merge_carries_the_two_new_kinds() {
        let provider = Provider::new("yandex").expect("a valid provider name");
        let one = Schema::new().monotonic(
            provider.clone(),
            Kind::new("disk").expect("a valid kind"),
            "size",
        );
        let two = Schema::new().stop_to_change(
            provider,
            Kind::new("instance").expect("a valid kind"),
            "cores",
        );

        let merged = one.merge(two);

        assert!(merged.grows_only(&address("yandex:disk.data"), "size"));
        assert!(merged.requires_stop(&address("yandex:instance.db"), "cores"));
    }
}
