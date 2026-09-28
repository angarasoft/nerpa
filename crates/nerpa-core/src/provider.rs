//! The boundary between the model and anything that touches the world.
//!
//! Expands: 0004
//!
//! Everything in this crate up to here is arithmetic on declarations. A provider
//! is where that stops: it reads what is actually there and makes it what it
//! should be. Nothing else in the workspace is allowed to.
//!
//! # Two things called provider
//!
//! [`address::Provider`](crate::address::Provider) is the *name* in an address —
//! the `yandex` in `yandex:instance.db`. The [`Provider`] trait here is the thing
//! that answers to that name. Related, not the same, and not re-exported
//! together at the crate root for that reason.
//!
//! # Every provider ships a fake
//!
//! [`Provider::fake`] is on the trait, so a provider without one does not
//! compile. Making providers testable by construction rather than by discipline
//! matters more here than usual, because most of them will be written by people
//! who are not reading this (`0015`).

use std::collections::{BTreeMap, BTreeSet};

use crate::address::{Address, Provider as ProviderName, Target};
use crate::observed::Facts;
use crate::plan::ChangeKind;
use crate::schema::Schema;
use crate::value::Value;
use nerpa_proto::{Operation, Premise, Seen};

/// What a provider carries a resource's state as.
pub type Attributes = BTreeMap<String, Value>;

/// One resource to look at, and what is known about where to find it.
///
/// Reads arrive in batches, and a batch never spans machines: the engine groups
/// by node before asking, so a provider can turn a whole batch into one journey.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reading {
    address: Address,
    locator: Attributes,
}

/// What reading a batch came back with.
///
/// Two things rather than one, because a provider is the only place that knows
/// both. It read a machine to decide what is there, and what it looked at is
/// what the plan will be computed *from* — so it says which observations those
/// were and what they hashed to, and the batch that acts carries them back.
/// Nothing else can produce this: the engine never sees a `Seen`, and a
/// provider rebuilding the observation later would be two guesses that have to
/// agree with nothing checking that they do. See decision 0034.
#[derive(Debug, Default)]
pub struct Read {
    attributes: BTreeMap<Address, Attributes>,
    premise: BTreeMap<Address, Vec<Premise>>,
}

impl Read {
    /// Nothing read yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// What one resource turned out to be, and what was looked at to find out.
    #[must_use]
    pub fn with(mut self, address: Address, attributes: Attributes, premise: Vec<Premise>) -> Self {
        self.attributes.insert(address.clone(), attributes);
        if !premise.is_empty() {
            self.premise.insert(address, premise);
        }
        self
    }

    /// What each resource turned out to be.
    #[must_use]
    pub fn attributes(&self) -> &BTreeMap<Address, Attributes> {
        &self.attributes
    }

    /// What was looked at to decide, for each resource that anything was looked
    /// at for.
    #[must_use]
    pub fn premise(&self) -> &BTreeMap<Address, Vec<Premise>> {
        &self.premise
    }
}

impl Reading {
    /// Names a resource and where to look for it.
    #[must_use]
    pub fn new(address: Address, locator: Attributes) -> Self {
        Self { address, locator }
    }

    /// Which resource.
    #[must_use]
    pub fn address(&self) -> &Address {
        &self.address
    }

    /// What the declaration or the last record says about where it is.
    ///
    /// An address does not always say where in the world to look — a file is
    /// found by its path, and that lives in the declaration. Only settled values
    /// are here; anything still waiting on another resource is left out, because
    /// a resource whose very location depends on something uncreated cannot be
    /// there yet.
    #[must_use]
    pub fn locator(&self) -> &Attributes {
        &self.locator
    }
}

/// Why a provider could not do what it was asked.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ProviderError {
    /// Nothing answers to this name.
    #[error("no provider is registered for {0}")]
    Unregistered(ProviderName),
    /// This provider does not handle resources of that kind.
    #[error("{provider} does not know how to handle {address}")]
    UnknownKind {
        /// The provider that was asked.
        provider: ProviderName,
        /// What it was asked about.
        address: Address,
    },
    /// A required attribute was missing, or held the wrong shape.
    #[error("{address}: {problem}")]
    Declaration {
        /// The resource whose declaration was wrong.
        address: Address,
        /// What was wrong with it.
        problem: String,
    },
    // ENFORCES: INV-REACH-002
    /// The machine could not be contacted at all.
    ///
    /// Deliberately separate from every other failure. Treating a permission
    /// error as unreachability would quietly stop managing a resource because of
    /// a typo in an access rule, and that is the class of silent wrongness this
    /// project exists to avoid. Only this variant lets a run continue.
    #[error("{node}: could not be reached: {problem}")]
    Unreachable {
        /// The machine that did not answer.
        node: Target,
        /// What was tried, and what came back.
        problem: String,
    },
    /// Something reached for a machine on behalf of a resource that is not on
    /// one.
    ///
    /// Without an address, because the thing that notices is the seam rather
    /// than anything holding a resource — and inventing one to fill the field
    /// would be a lie about what went wrong.
    #[error("this resource is not on a machine, so there is nothing to ask of one")]
    NotOnAMachine,
    /// The world refused.
    #[error("{address}: {problem}")]
    World {
        /// The resource being read or changed.
        address: Address,
        /// What went wrong.
        problem: String,
    },
}

/// The kinds of change that send something.
///
/// An adoption and a reorder write state down and nothing else, so neither ever
/// reaches a provider ([`ChangeKind::sends_anything`]). Kept apart from
/// [`ChangeKind`] for that reason: what a provider is asked to build is the
/// smaller set, and a provider that has to handle a kind it can never be given
/// is a provider with untested arms in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum Sending {
    /// It does not exist, and will.
    Create,
    /// It exists, and some attribute changes in place.
    Update,
    /// It exists, and what changed cannot change in place: what is there goes,
    /// and a new one takes its address.
    Replace,
    /// It exists and is no longer declared, so it goes.
    Delete,
    /// It matches its declaration, and something it reacts to changed.
    React,
}

impl Sending {
    /// What a planned change sends, or `None` where nothing is sent.
    #[must_use]
    pub fn of(kind: ChangeKind) -> Option<Self> {
        match kind {
            ChangeKind::Create => Some(Self::Create),
            ChangeKind::Update => Some(Self::Update),
            ChangeKind::Replace => Some(Self::Replace),
            ChangeKind::Delete => Some(Self::Delete),
            ChangeKind::React => Some(Self::React),
            ChangeKind::Adopt | ChangeKind::Reorder => None,
        }
    }
}

/// Everything a provider is told in order to build one change.
///
/// No connection among it: a provider decides what to send, and sending it is
/// somebody else's job.
#[derive(Debug)]
pub struct Prepared<'a> {
    /// The resource being changed.
    pub address: &'a Address,
    /// What is happening to it.
    pub kind: Sending,
    /// Something it reacts to changed, or an earlier run left that reaction
    /// unpaid.
    pub roused: bool,
    /// Sent again: an earlier run marked this change and never recorded it, and
    /// this run's reading matches the declaration, so the same change goes
    /// again rather than being worked out afresh.
    pub resent: bool,
    /// Settled values, secrets included; empty for a removal.
    pub desired: &'a Attributes,
    /// Which of those names are secrets — the names, never which value is which
    /// kind of thing beyond that. A provider marks the operation that carries a
    /// secret, and the engine refuses a change where the marking and this
    /// disagree, so that two claims have to line up rather than one being
    /// trusted.
    pub secret: &'a BTreeSet<String>,
    /// What state recorded, for a replacement and a removal.
    pub recorded: Option<&'a Attributes>,
    /// What the machine said about itself.
    pub facts: Facts<'a>,
}

/// What a provider is told when its change is over: the change with no secret
/// value in it.
///
/// A secret lives inside the operation that carries it until that operation is
/// written, and nothing downstream of sending ever holds one. See 0027.
#[derive(Debug)]
pub struct Concluding<'a> {
    /// The resource that was changed.
    pub address: &'a Address,
    /// What happened to it.
    pub kind: Sending,
    /// Whether it was roused as well.
    pub roused: bool,
    /// What the machine said about itself.
    pub facts: Facts<'a>,
    /// Settled values that are not secrets.
    pub plain: &'a Attributes,
    /// The names of those that were.
    pub secret: &'a BTreeSet<String>,
}

/// The operations one change is made of.
#[derive(Debug, Default, Clone)]
pub struct Planned {
    /// What to send, in order. One change's operations are contiguous: they are
    /// never interleaved with another change's.
    pub operations: Vec<Operation>,
    /// For a replacement, how many of the operations remove what was there. The
    /// resource is recorded as forgotten after them, because between the two
    /// halves it is gone and nothing is there yet. `None` for every other kind.
    pub forget_after: Option<usize>,
    /// For each operation that writes, the declared attributes its answer
    /// confirms. Counted over the operations of the whole change, a
    /// replacement's two halves included.
    pub carries: BTreeMap<usize, BTreeSet<String>>,
}

/// One operation's answer, as the engine hands it to a provider.
///
/// The engine's shape, not the wire's. It exists so that what a provider is told
/// can differ from what came back: a write is answered with a digest the engine
/// checks against what it sent, and the provider is told that it matched rather
/// than handed the bytes again.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Answered {
    /// What the far end said it saw.
    Saw(Seen),
    /// A write whose answer the engine checked against what it sent, and which
    /// matched. What it confirms is what [`Planned::carries`] named for that
    /// operation — so a provider is told that the write took, without being
    /// handed the bytes again or, for a secret, at all.
    Written,
}

/// What a change turned out to be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Concluded {
    /// It is there, and these are its attributes.
    Became(Attributes),
    /// It is not there any more, and state forgets it.
    Gone,
}

/// What the next run does with a change that a run marked and never recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Afterwards {
    /// Reading sees every declared attribute the change sets, so the next run
    /// reads what is there and decides from it.
    Read,
    /// Reading cannot tell whether the change happened, so it is sent again.
    SendAgain,
}

/// Reads and changes resources of the kinds it owns.
// ENFORCES: INV-PROVIDER-001
// ENFORCES: INV-PROVIDER-002
// `read` is the only method here that is handed a connection. What a change is
// made of is said, not done: the engine sends it.
#[slipway_derive::doc_anchor(id = "provider-trait")]
pub trait Provider: Send + Sync {
    /// The name this provider answers to in an address.
    fn name(&self) -> &ProviderName;

    /// What this provider refuses to change in place.
    fn schema(&self) -> Schema;

    /// What these resources look like now.
    ///
    /// Only what exists comes back; an address absent from the answer does not
    /// exist. Not existing is an answer, not a failure.
    ///
    // ENFORCES: INV-BATCH-001
    /// **A batch, not one at a time**, and that is the whole shape of it. The
    /// difference between one journey to a machine and one per resource is what
    /// makes Ansible slow, and an interface that can only ask about a single
    /// resource makes the fast version unrepresentable however good the transport
    /// underneath is. A provider with nothing to gain may simply loop.
    ///
    /// The engine groups by machine before calling, so every batch belongs to one
    /// node — which is what lets [`ProviderError::Unreachable`] speak for the
    /// whole of it.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] if the batch could not be read at all.
    fn read(
        &self,
        batch: &[Reading],
        facts: Facts<'_>,
        reach: &dyn Reach,
    ) -> Result<Read, ProviderError>;

    /// The operations that make this change, and what their answers confirm.
    ///
    /// No machine among the arguments, which is the whole of the rule: a
    /// provider says what to send, and the engine sends it. A replacement is one
    /// call — the operations that remove what is there, then the ones that make
    /// what should be, with [`Planned::forget_after`] between them.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] if the declaration cannot be turned into
    /// operations: an attribute missing, a kind this provider does not own, or a
    /// machine that did not say what it is.
    fn prepare(&self, change: &Prepared<'_>) -> Result<Planned, ProviderError>;

    /// What the change became, from the answers to exactly its own operations.
    ///
    /// Handed every answer of the change, in the order it asked for them, once
    /// the last of them has run. What it reports is what state records.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] if the answers say the change did not take: a
    /// path not there after writing it, a package still absent after installing
    /// it, a service that will not stay up.
    fn conclude(
        &self,
        change: &Concluding<'_>,
        became: &[Answered],
    ) -> Result<Concluded, ProviderError>;

    /// What the next run does with this change if a run marked it and stopped
    /// before recording it.
    ///
    /// [`Afterwards::Read`] wherever reading sees every declared attribute the
    /// change sets, which is most kinds. [`Afterwards::SendAgain`] where it does
    /// not: a package is registered before its post-install has run, so finding
    /// it installed does not mean the change finished.
    fn afterwards(&self, address: &Address, kind: Sending) -> Afterwards;

    /// A version of this provider that touches nothing.
    ///
    /// Required rather than encouraged: a provider whose behaviour can only be
    /// exercised against the real world is a provider nobody will test.
    fn fake() -> Self
    where
        Self: Sized;
}

/// Everything that can be asked about a resource, by provider name.
#[derive(Default)]
pub struct Providers {
    registered: BTreeMap<ProviderName, Box<dyn Provider>>,
}

impl std::fmt::Debug for Providers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Providers")
            .field("registered", &self.registered.keys())
            .finish()
    }
}

impl Providers {
    /// A registry that knows nobody.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a provider under the name it answers to.
    #[must_use]
    pub fn with(mut self, provider: Box<dyn Provider>) -> Self {
        self.registered.insert(provider.name().clone(), provider);
        self
    }

    /// The provider for an address.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::Unregistered`] if nothing answers to that name.
    /// Failing closed: a resource nobody owns cannot be read, and pretending it
    /// does not exist would propose creating it for ever.
    pub fn named(&self, name: &ProviderName) -> Result<&dyn Provider, ProviderError> {
        self.registered
            .get(name)
            .map(AsRef::as_ref)
            .ok_or_else(|| ProviderError::Unregistered(name.clone()))
    }

    /// The provider for an address.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::Unregistered`] if nothing answers to that name.
    pub fn for_address(&self, address: &Address) -> Result<&dyn Provider, ProviderError> {
        self.registered
            .get(address.provider())
            .map(AsRef::as_ref)
            .ok_or_else(|| ProviderError::Unregistered(address.provider().clone()))
    }

    /// Every schema, merged.
    #[must_use]
    pub fn schema(&self) -> Schema {
        self.registered
            .values()
            .fold(Schema::new(), |merged, provider| {
                merged.merge(provider.schema())
            })
    }

    /// How many providers are registered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.registered.len()
    }

    /// Whether nothing is registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.registered.is_empty()
    }
}

/// A way to reach the machine a batch is about.
///
/// Provider logic runs here and the work happens there
/// (`0032`); this is the seam. A provider composes what
/// it wants looked at or done, hands it over, and reads the answers — it does
/// not know whether the far end was reached by a pipe, by `ssh`, or by an
/// identity-aware proxy, and it must not care.
///
/// A provider whose resources live in the `control` domain never uses one. There
/// is no machine to reach: a cloud API is reached from where the engine already
/// is.
pub trait Reach: std::fmt::Debug + Send + Sync {
    /// Asks, and waits for the answer.
    ///
    /// One journey. The batch is the unit precisely so this is called once per
    /// machine per phase rather than once per resource — see `INV-BATCH-001`.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::Unreachable`] when the machine could not be
    /// contacted at all — the one failure a run survives — and
    /// [`ProviderError::World`] for anything else.
    fn ask(&self, ask: &nerpa_proto::Ask) -> Result<nerpa_proto::Answer, ProviderError>;
}

/// How each machine is reached, and what answers for anything not on one.
#[derive(Debug, Default)]
pub struct Machines {
    reaches: BTreeMap<Target, Box<dyn Reach>>,
    anywhere: Option<Box<dyn Reach>>,
}

impl Machines {
    /// Reaching nothing yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// And now reaching this one.
    #[must_use]
    pub fn with(mut self, node: Target, reach: Box<dyn Reach>) -> Self {
        self.reaches.insert(node, reach);
        self
    }

    /// Reaching every machine the same way.
    ///
    /// True when every node is this one — which it is for a local run and for
    /// most tests. Opt-in rather than the default, because without it a node
    /// nothing knows how to reach says so, and that refusal is worth keeping.
    #[must_use]
    pub fn everywhere(mut self, reach: Box<dyn Reach>) -> Self {
        self.anywhere = Some(reach);
        self
    }

    /// What to reach this resource's machine by.
    ///
    /// A resource in the `control` domain has no machine, and gets something
    /// that refuses rather than an `Option` every provider would have to unwrap.
    /// A cloud provider never asks; one that does is told plainly why it cannot.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::Unreachable`] for a machine nothing knows how to
    /// reach. Deliberately the same failure as one that did not answer: from
    /// everywhere upstream, a node with no route to it and a node that is down
    /// are the same fact.
    pub fn reach(&self, node: Option<&Target>) -> Result<&dyn Reach, ProviderError> {
        let Some(node) = node else {
            return Ok(&Nowhere);
        };
        self.reaches
            .get(node)
            .or(self.anywhere.as_ref())
            .map(AsRef::as_ref)
            .ok_or_else(|| ProviderError::Unreachable {
                node: node.clone(),
                problem: "nothing knows how to reach it".to_owned(),
            })
    }
}

/// What answers for a resource that is not on a machine.
///
/// Not a null object doing nothing quietly: it refuses, and says why. A
/// `control` provider reaching for a machine is a provider confused about which
/// domain its resources are in, and that is worth being told.
#[derive(Debug)]
struct Nowhere;

impl Reach for Nowhere {
    fn ask(&self, _ask: &nerpa_proto::Ask) -> Result<nerpa_proto::Answer, ProviderError> {
        Err(ProviderError::NotOnAMachine)
    }
}
