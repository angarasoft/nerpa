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
//! reference to a secret store and a digest of what is behind it — is
//! [`Recorded`], and it is the type of what a [`Record`] holds. There is no
//! path from a stored digest back to a value, so this is true by construction
//! rather than by everyone remembering. See decision 0027.

use std::collections::{BTreeMap, BTreeSet};

use crate::address::Address;
use crate::provider::Attributes;
use crate::secret::Recorded;

/// What state holds for one resource: values, and digests where the value was a
/// secret.
pub type Recording = BTreeMap<String, Recorded>;

/// What one resource looked like when Nerpa last touched it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    attributes: Recording,
    depends_on: BTreeSet<Address>,
    /// What this resource was roused by and has not yet acted on.
    ///
    /// Written down before the action is attempted and gone once it succeeds.
    /// A run that stops in between leaves the debt, and the next plan shows it
    /// — because the alternative is a new file, an old process, and nothing
    /// recording that fact. See decision 0026.
    owed: BTreeSet<Address>,
    written: Written,
}

/// When a record was written, in seconds since the epoch.
///
/// The only value in the model that comes from outside it, and it is fenced in on
/// purpose. It exists to answer one question — how old is what we know about a
/// machine that is not answering — and it must never take part in a comparison
/// during planning, or resources would start to differ from one another because a
/// minute passed. See decision 0020.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Written(u64);

impl Written {
    /// Reads the clock.
    #[must_use]
    pub fn now() -> Self {
        Self(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |since| since.as_secs()),
        )
    }

    /// A moment given as seconds since the epoch.
    #[must_use]
    pub fn at(seconds: u64) -> Self {
        Self(seconds)
    }

    /// Seconds since the epoch.
    #[must_use]
    pub fn seconds(self) -> u64 {
        self.0
    }

    /// How long ago this was, measured from `now`.
    ///
    /// Saturating: a record written in the future means a clock moved backwards,
    /// and reporting a negative age would be less useful than reporting none.
    #[must_use]
    pub fn age(self, now: Self) -> std::time::Duration {
        std::time::Duration::from_secs(now.0.saturating_sub(self.0))
    }
}

impl Record {
    /// Records a resource and what it depended on, as of now.
    #[must_use]
    pub fn new(attributes: Recording, depends_on: BTreeSet<Address>) -> Self {
        Self {
            attributes,
            depends_on,
            owed: BTreeSet::new(),
            written: Written::now(),
        }
    }

    /// Records a resource as of a stated moment, for reading state back from
    /// disk and for tests that need a fixed clock.
    #[must_use]
    pub fn written_at(
        attributes: Recording,
        depends_on: BTreeSet<Address>,
        written: Written,
    ) -> Self {
        Self {
            attributes,
            depends_on,
            owed: BTreeSet::new(),
            written,
        }
    }

    /// The same, owing a reaction that has not been paid.
    #[must_use]
    pub fn owing(mut self, owed: BTreeSet<Address>) -> Self {
        self.owed = owed;
        self
    }

    /// What this resource was roused by and has not yet acted on.
    #[must_use]
    pub fn owed(&self) -> &BTreeSet<Address> {
        &self.owed
    }

    /// When this was written.
    #[must_use]
    pub fn written(&self) -> Written {
        self.written
    }

    /// What the provider reported after the last change, with secrets reduced
    /// to digests.
    #[must_use]
    pub fn attributes(&self) -> &Recording {
        &self.attributes
    }

    /// The attributes a provider may be handed back — the ones that are values.
    ///
    /// A secret is not among them, and cannot be: state never held it. Anything
    /// that needs one has to fetch it from the store again, which is the whole
    /// point of not keeping it here.
    // ENFORCES: INV-SECRET-003
    #[must_use]
    pub fn locator(&self) -> Attributes {
        self.attributes
            .iter()
            .filter_map(|(name, recorded)| Some((name.clone(), recorded.plain()?.clone())))
            .collect()
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

    /// Writes down that a resource owes a reaction, before its cause is sent.
    ///
    /// The order is the whole point. A debt written after the action would be
    /// lost by exactly the run that failed to pay it, which is the state
    /// Ansible reaches: a new file, an old process, and no record. Written
    /// first, the window between the two resolves toward reporting a debt that
    /// was in fact paid — one redundant restart, against an outage nobody knows
    /// about. See decision 0026.
    pub fn owe(&mut self, address: &Address, causes: &[Address]) {
        if let Some(record) = self.resources.get_mut(address) {
            record.owed.extend(causes.iter().cloned());
        }
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
