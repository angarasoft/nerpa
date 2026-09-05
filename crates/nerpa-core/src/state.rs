//! What the last run left behind.
//!
//! Observed state answers "what is there now". State answers something no
//! amount of looking can: **what Nerpa made, and what it depended on when it
//! made it.** Two things need that and neither can be worked out from the world:
//!
//! * A resource that is no longer declared has to be removed, and removing it in
//!   the wrong order takes a network down before the machine on it. The order
//!   comes from the edges recorded when it was last applied — the configuration
//!   that described them is gone.
//! * A resource whose location changed has to be found where it used to be.
//!   Looking where the new declaration points finds nothing and proposes making
//!   a second one.
//!
//! # What it deliberately does not hold
//!
//! Secret values. Terraform's state carries passwords in clear text, which is
//! why it needs whole-file encryption and still leaks through every backup. A
//! state that never held the value has nothing to leak. The mechanism — a
//! reference to a secret store and a digest of what is behind it — arrives with
//! the `Attribute::Secret` variant; until a declaration can say that an
//! attribute is secret, nothing here is.

use std::collections::{BTreeMap, BTreeSet};

use crate::address::Address;
use crate::provider::Attributes;

/// What one resource looked like when Nerpa last touched it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    attributes: Attributes,
    depends_on: BTreeSet<Address>,
}

impl Record {
    /// Records a resource and what it depended on at the time.
    #[must_use]
    pub fn new(attributes: Attributes, depends_on: BTreeSet<Address>) -> Self {
        Self {
            attributes,
            depends_on,
        }
    }

    /// What the provider reported after the last change.
    #[must_use]
    pub fn attributes(&self) -> &Attributes {
        &self.attributes
    }

    /// What it depended on then — which is the only thing that can order its
    /// removal now.
    #[must_use]
    pub fn depends_on(&self) -> &BTreeSet<Address> {
        &self.depends_on
    }
}

/// Everything Nerpa has made and not yet removed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct State {
    resources: BTreeMap<Address, Record>,
}

impl State {
    /// A state that remembers nothing, which is what a first run starts from.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records what a resource became.
    pub fn record(&mut self, address: Address, record: Record) {
        self.resources.insert(address, record);
    }

    /// Forgets a resource, once it is gone from the world too.
    pub fn forget(&mut self, address: &Address) {
        self.resources.remove(address);
    }

    /// What is remembered about a resource.
    #[must_use]
    pub fn get(&self, address: &Address) -> Option<&Record> {
        self.resources.get(address)
    }

    /// Every address Nerpa believes it made, in a stable order.
    pub fn addresses(&self) -> impl Iterator<Item = &Address> {
        self.resources.keys()
    }

    /// How many resources are remembered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.resources.len()
    }

    /// Whether nothing is remembered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.resources.is_empty()
    }

    /// The remembered resources that `declared` no longer contains, in the order
    /// they have to be removed.
    ///
    /// Reverse dependency order: whatever depended on a thing goes before the
    /// thing it depended on. The edges come from what was recorded, because the
    /// configuration that described them is exactly what was deleted.
    ///
    /// A cycle among remembered edges cannot happen — they were applied in some
    /// order once — so one is treated as a flat set rather than an error nobody
    /// could act on.
    // ENFORCES: INV-STATE-002
    #[must_use]
    pub fn abandoned(&self, declared: &BTreeSet<&Address>) -> Vec<Vec<Address>> {
        let doomed: BTreeSet<&Address> = self
            .resources
            .keys()
            .filter(|address| !declared.contains(*address))
            .collect();

        let mut remaining: BTreeMap<&Address, BTreeSet<&Address>> = doomed
            .iter()
            .map(|address| {
                let blockers = self
                    .resources
                    .get(*address)
                    .map(|record| {
                        // Only doomed dependencies hold up a removal. Anything
                        // still declared is staying, and waiting for it would
                        // wait for ever.
                        record
                            .depends_on
                            .iter()
                            .filter(|other| doomed.contains(other))
                            .collect()
                    })
                    .unwrap_or_default();
                (*address, blockers)
            })
            .collect();

        let mut levels = Vec::new();
        while !remaining.is_empty() {
            // Removable now: nothing still on the list reads from them. The
            // reverse of the order they were made in — the other way round takes
            // the directory out from under the file.
            let ready: Vec<&Address> = remaining
                .keys()
                .filter(|address| {
                    !remaining
                        .values()
                        .any(|depends_on| depends_on.contains(*address))
                })
                .copied()
                .collect();

            if ready.is_empty() {
                // Applied once, so orderable once; a loop here means the record
                // is inconsistent, and removing them together beats not at all.
                levels.push(remaining.keys().map(|address| (*address).clone()).collect());
                break;
            }

            for address in &ready {
                remaining.remove(address);
            }
            levels.push(ready.into_iter().cloned().collect());
        }

        levels
    }
}
