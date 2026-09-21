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

use crate::address::{Address, Target};
use crate::plan::ChangeKind;
use crate::provider::Afterwards;
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
#[derive(
    serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
#[serde(transparent)]
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

/// Where a mark stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Standing {
    /// Written before the request went out and not yet answered. Only a mark in
    /// flight holds its machine: a run that ended while one stood here left a
    /// change whose outcome nobody knows.
    InFlight,
    /// A change that failed after its operations ran, and whose provider says
    /// reading cannot tell whether it took. It is sent again.
    Pending,
    /// The same, for a change reading *can* resolve. Kept only so that what the
    /// failed change may have made can be removed if its declaration goes.
    ForRemovalOnly,
}

/// What a change leaves behind from the moment it is sent until its answer is
/// written down.
///
/// Written before the request, because a request sent without one is a change
/// nothing records: the run that stops in between leaves a machine that was
/// acted on and a state file that says it was not. Settled by the write that
/// records the answer.
///
/// It carries no secret value. What it keeps of the change is its locator and
/// the settled values that were not secrets, which is what the next run needs to
/// decide what to do about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mark {
    kind: ChangeKind,
    locator: Attributes,
    shown: Attributes,
    depends_on: BTreeSet<Address>,
    written: Written,
    past_forgetting: bool,
    afterwards: Afterwards,
    standing: Standing,
    replaced: Option<Box<Mark>>,
}

impl Mark {
    /// A mark for a change about to be sent.
    #[must_use]
    pub fn new(kind: ChangeKind, locator: Attributes, shown: Attributes) -> Self {
        Self {
            kind,
            locator,
            shown,
            depends_on: BTreeSet::new(),
            written: Written::now(),
            past_forgetting: false,
            afterwards: Afterwards::Read,
            standing: Standing::InFlight,
            replaced: None,
        }
    }

    /// What the change comes after, so that a removal built from this mark can
    /// be ordered as the record would have been.
    #[must_use]
    pub fn after(mut self, depends_on: BTreeSet<Address>) -> Self {
        self.depends_on = depends_on;
        self
    }

    /// What its provider said the next run could do about it, asked when the
    /// mark was written rather than when it is read: the provider may be gone by
    /// then, and the answer belongs to the change that was sent.
    #[must_use]
    pub fn afterwards(mut self, afterwards: Afterwards) -> Self {
        self.afterwards = afterwards;
        self
    }

    /// A replacement whose removal half is done: what was there is gone, and the
    /// record with it.
    #[must_use]
    pub fn past_forgetting(mut self) -> Self {
        self.past_forgetting = true;
        self
    }

    /// Where it stands, for a mark being written as an entry rather than in
    /// flight.
    #[must_use]
    pub fn standing(mut self, standing: Standing) -> Self {
        self.standing = standing;
        self
    }

    /// The entry this mark was written over, if it was written over one.
    ///
    /// Kept because releasing a machine may have to put it back: a mark that
    /// says less than the entry it replaced would otherwise lose what the entry
    /// knew.
    #[must_use]
    pub fn replacing(mut self, replaced: Mark) -> Self {
        self.replaced = Some(Box::new(replaced));
        self
    }

    /// The same, from what was written down.
    #[must_use]
    pub fn written_at(mut self, written: Written) -> Self {
        self.written = written;
        self
    }

    /// What kind of change it was.
    #[must_use]
    pub fn kind(&self) -> ChangeKind {
        self.kind
    }

    /// Where the resource is, as far as this change knew.
    #[must_use]
    pub fn locator(&self) -> &Attributes {
        &self.locator
    }

    /// The settled values that were not secrets.
    #[must_use]
    pub fn shown(&self) -> &Attributes {
        &self.shown
    }

    /// What it came after.
    #[must_use]
    pub fn depends_on(&self) -> &BTreeSet<Address> {
        &self.depends_on
    }

    /// When it was written.
    #[must_use]
    pub fn written(&self) -> Written {
        self.written
    }

    /// Whether the removal half of a replacement is already done.
    #[must_use]
    pub fn is_past_forgetting(&self) -> bool {
        self.past_forgetting
    }

    /// What its provider said the next run could do.
    #[must_use]
    pub fn afterwards_said(&self) -> Afterwards {
        self.afterwards
    }

    /// Where it stands.
    #[must_use]
    pub fn stands(&self) -> Standing {
        self.standing
    }

    /// Whether it is still in flight, which is what holds a machine.
    #[must_use]
    pub fn in_flight(&self) -> bool {
        self.standing == Standing::InFlight
    }

    /// The entry it was written over.
    #[must_use]
    pub fn replaced(&self) -> Option<&Mark> {
        self.replaced.as_deref()
    }

    /// What this mark becomes once nobody will ever learn whether its request
    /// arrived: an entry, by what its provider said reading could resolve.
    ///
    /// A change reading cannot resolve is sent again; one it can is kept only so
    /// that what the change may have made can be removed if its declaration
    /// goes. An entry this mark was written over comes back where it says more
    /// than that — a resend that must still happen outlives a record of
    /// something to remove by.
    #[must_use]
    pub fn settled(self) -> Self {
        let standing = match self.afterwards {
            Afterwards::SendAgain => Standing::Pending,
            Afterwards::Read => Standing::ForRemovalOnly,
        };
        if let Some(replaced) = &self.replaced
            && replaced.standing == Standing::Pending
            && standing == Standing::ForRemovalOnly
        {
            return (**replaced).clone();
        }
        self.standing(standing)
    }
}

/// Everything Nerpa has made and not yet removed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct State {
    resources: BTreeMap<Address, Record>,
    /// At most one for each address: a change in flight, or an entry a failed
    /// one left. The next mark for an address replaces what is there.
    marks: BTreeMap<Address, Mark>,
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

    /// Writes down that a change is about to be sent, or the entry a failed one
    /// left.
    ///
    /// At most one for each address: the next one replaces what is there, which
    /// is why a mark carries the entry it replaced.
    pub fn mark(&mut self, address: Address, mark: Mark) {
        self.marks.insert(address, mark);
    }

    /// The mark for this address, if it has one.
    #[must_use]
    pub fn marked(&self, address: &Address) -> Option<&Mark> {
        self.marks.get(address)
    }

    /// Every mark, by address.
    pub fn marks(&self) -> impl Iterator<Item = (&Address, &Mark)> {
        self.marks.iter()
    }

    /// Takes a mark away, which is what the write recording an answer does.
    pub fn settle(&mut self, address: &Address) -> Option<Mark> {
        self.marks.remove(address)
    }

    /// Settles every change left in flight on one machine, or on the control
    /// domain, and says how many there were.
    ///
    /// What a person does when a run was killed: nothing here can tell whether
    /// the changes arrived, so somebody who can look at the machine says it is
    /// theirs again. What was in flight is not thrown away — it becomes what the
    /// next plan has to deal with, because something may be out there that only
    /// the entry remembers. RFC 0001 §5.8.
    pub fn release(&mut self, domain: Option<&Target>) -> usize {
        let held: Vec<Address> = self
            .marks
            .iter()
            .filter(|(address, mark)| mark.in_flight() && address.target() == domain)
            .map(|(address, _)| (*address).clone())
            .collect();

        let settled = held.len();
        for address in held {
            if let Some(mark) = self.marks.remove(&address) {
                self.marks.insert(address, mark.settled());
            }
        }
        settled
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

    /// What this address came after, as a record says, or as the entry of a
    /// change that never got a record does.
    fn after(&self, address: &Address) -> Option<&BTreeSet<Address>> {
        self.resources
            .get(address)
            .map(Record::depends_on)
            .or_else(|| self.marks.get(address).map(Mark::depends_on))
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
        // Entries as well as records. A change that failed after its operations
        // ran has no record — the record is written from what it became, and it
        // never became anything — so the entry is the only thing that knows
        // something may be out there under this address.
        let doomed: BTreeSet<&Address> = self
            .resources
            .keys()
            .chain(
                self.marks
                    .iter()
                    .filter(|(_, mark)| !mark.in_flight())
                    .map(|(address, _)| address),
            )
            .filter(|address| !declared.contains(*address))
            .collect();

        let mut remaining: BTreeMap<&Address, BTreeSet<&Address>> = doomed
            .iter()
            .map(|address| {
                let blockers = self
                    .after(address)
                    .map(|depends_on| {
                        // Only doomed dependencies hold up a removal. Anything
                        // still declared is staying, and waiting for it would
                        // wait for ever.
                        depends_on
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
