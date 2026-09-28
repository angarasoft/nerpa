//! What a run intends to do, computed in full before it does any of it.
//!
//! A plan is the artefact of record. It is what a person reads, what an agent
//! decides from, and — once the hash lands — what somebody signs. So its shape
//! is settled now rather than grown later: everything here is ordered, nothing
//! is a set with an arbitrary iteration order, and no field means "we will work
//! this out during apply".
//!
//! # What is deliberately absent
//!
//! **Deletions.** A resource that exists and is no longer declared has to go,
//! but it cannot be ordered without knowing what depended on it — and that
//! knowledge lives in state, which does not exist yet. Ordering a deletion by
//! guesswork removes a network before the machine on it. So deletions wait for
//! state rather than being approximated here.
//!
//! **Finding a resource that moved.** [`ChangeKind::Replace`] is computed
//! correctly from observed state, but observing it depends on knowing where the
//! resource is now — and for anything whose location is itself the attribute
//! that changed, that is the same missing knowledge. Until state remembers where
//! a resource used to be, moving one reads as creating another.

use std::collections::BTreeSet;

use crate::address::{Address, Target};
use crate::attribute::{Choice, Reference};
use crate::graph::StageKind;
use std::fmt;

use crate::secret::{Recorded, Sealed, Source};
use crate::state::Written;
use crate::value;
use crate::value::Value;

/// What is about to happen to one resource.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ChangeKind {
    /// It does not exist, and will.
    Create,
    /// It exists, and some attribute changes in place.
    Update,
    /// It exists, and an attribute that cannot change in place has changed, so
    /// it goes and a new one takes its address.
    Replace,
    /// It exists and is no longer declared, so it goes.
    Delete,
    /// It exists and matches its declaration, and something it reacts to
    /// changed — so it has to act anyway. A restart is the usual one.
    ///
    /// In the plan rather than at the end of a run, because the expensive part
    /// of a fleet-wide change is bouncing a service on every machine in it, and
    /// the way to find that out should not be to run it. See decision 0026.
    React,
    /// It exists and matches its declaration, and nothing records it — so
    /// nothing is done to it, and it is written down.
    ///
    /// In the plan because it changes what a later plan may do: from here on,
    /// taking the declaration away removes it, the same as for something Nerpa
    /// made. A resource that already matched on the first run used to be planned
    /// as nothing, never recorded, and never removable. See decision 0042.
    Adopt,
    /// It exists, matches its declaration and is recorded, and what it comes
    /// after is not what state remembers — so nothing is done to it, and it is
    /// recorded again with the edges it has now.
    ///
    /// In the plan for the same reason as an adoption: it changes what a later
    /// plan does, here the order its removal takes. State took edges only from
    /// what was applied, so an `after` written in once a file matched was never
    /// recorded, and the file went with its directory in any order. See decision
    /// 0043.
    Reorder,
}

impl ChangeKind {
    /// Whether the resource that is there now survives this.
    #[must_use]
    pub fn is_reversible(self) -> bool {
        // A restart destroys nothing and cannot be taken back either. It is
        // marked for the second reason: a reader scanning a plan over two
        // hundred machines is entitled to see it before it happens.
        matches!(
            self,
            ChangeKind::Create | ChangeKind::Update | ChangeKind::Adopt | ChangeKind::Reorder
        )
    }

    /// Whether anything is sent to where the resource lives.
    ///
    /// An adoption and a reorder only write state: the resource already is what
    /// it should be. So neither wants a provider or a connection, and neither is
    /// a change anything reacts to, because nothing on the machine moved.
    #[must_use]
    pub fn sends_anything(self) -> bool {
        match self {
            ChangeKind::Create
            | ChangeKind::Update
            | ChangeKind::Replace
            | ChangeKind::Delete
            | ChangeKind::React => true,
            ChangeKind::Adopt | ChangeKind::Reorder => false,
        }
    }
}

/// A value the plan is about to write, and whether it is known yet.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Confidence {
    /// Settled: this exact value will be written.
    Known(Value),
    /// Not settled at plan time, and will be read from another resource once
    /// that resource exists.
    Unknown(Reference),
    /// Settled, and deliberately not written here: a secret is carried as where
    /// it came from and what it hashes to. See decision 0027.
    Secret(Sealed),
    /// One of a written-down set, and which one is not settled yet.
    ///
    /// More useful than "unknown after apply", which names no possibilities at
    /// all: a person reading this knows every value it could take and can
    /// approve or refuse the set. See decision 0029.
    Bounded(Choice),
}

impl Confidence {
    /// Whether this is something that must not be written down.
    #[must_use]
    pub fn is_secret(&self) -> bool {
        matches!(self, Confidence::Secret(_))
    }
}

/// One attribute changing.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AttributeDelta {
    name: String,
    before: Option<Recorded>,
    after: Confidence,
    forces_replacement: bool,
}

impl AttributeDelta {
    pub(crate) fn new(
        name: String,
        before: Option<Recorded>,
        after: Confidence,
        forces_replacement: bool,
    ) -> Self {
        Self {
            name,
            before,
            after,
            forces_replacement,
        }
    }

    /// Which attribute.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// What it holds now, or `None` if it holds nothing.
    #[must_use]
    pub fn before(&self) -> Option<&Recorded> {
        self.before.as_ref()
    }

    /// What it will hold.
    #[must_use]
    pub fn after(&self) -> &Confidence {
        &self.after
    }

    /// Whether this attribute is the reason the resource is being replaced.
    #[must_use]
    pub fn forces_replacement(&self) -> bool {
        self.forces_replacement
    }
}

/// Everything about to happen to one resource.
///
/// Carries the whole desired state, not only what differs. A plan has to be
/// applicable on its own — that is what makes it worth signing, and what lets it
/// be handed to something that never saw the configuration. Deltas are for
/// reading; `desired` is for doing.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Change {
    address: Address,
    kind: ChangeKind,
    deltas: Vec<AttributeDelta>,
    desired: Vec<(String, Confidence)>,
    /// What the machine looked like when this was decided, and what that hashed
    /// to. Deliberately outside the plan's number: the number covers what the
    /// plan intends to do, and this covers what it reasoned from, which is a
    /// different question and belongs to a different check. See `INV-PLAN-006`
    /// and decision 0034.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    premise: Vec<nerpa_proto::Premise>,
    /// What changed that this resource reacts to.
    ///
    /// Carried whatever the kind turned out to be: a service whose own
    /// attributes changed as well is one `Update`, not an update and a restart,
    /// and the provider still has to be told why it is being asked.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    roused_by: Vec<Address>,
    /// What roused it in an earlier run that did not finish paying.
    ///
    /// Separate from `roused_by` because they are different facts and a reader
    /// is owed the difference: one says a file is changing now, the other says
    /// a restart was owed since a run that stopped. See decision 0026.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    owed: Vec<Address>,
    /// Reacting resources this plan cannot reach, which this change rouses.
    ///
    /// Owed a reaction on their records before this is sent, and paid in
    /// whichever later run they can be planned in. Carried by the cause,
    /// because the reactor is not in the plan to carry it. See `INV-REACT-006`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    owed_by: Vec<Address>,
    /// Everything this resource comes after, however that ordering arose: a
    /// reference, `after`, a reaction.
    ///
    /// Carried so that state can remember it. Removal order comes from state,
    /// because the configuration that described a dependency is exactly what was
    /// deleted, and state used to keep only the references still unresolved when
    /// planning — so a dependency that already existed, or one written as `after`,
    /// was forgotten and its removal came in any order. See `INV-STATE-002`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    depends_on: Vec<Address>,
    /// Sent again: an earlier run marked this change, never recorded it, and
    /// this run's reading leaves the declaration's own change with nothing to
    /// send. See `crate::state::Mark`.
    ///
    /// Encoded for every change, false or not. What the encoding covers must
    /// not depend on what state happened to hold, or two plans that intend
    /// different things could agree on a number.
    resent: bool,
    /// The change this repeats had passed its forget point: for a replacement,
    /// the removal half is done and what is left is the create.
    past_forgetting: bool,
}

impl Change {
    #[allow(
        clippy::too_many_arguments,
        reason = "one argument for each part of a change, so none can be left out"
    )]
    pub(crate) fn new(
        address: Address,
        kind: ChangeKind,
        deltas: Vec<AttributeDelta>,
        desired: Vec<(String, Confidence)>,
        premise: Vec<nerpa_proto::Premise>,
        roused_by: Vec<Address>,
        owed: Vec<Address>,
        depends_on: Vec<Address>,
    ) -> Self {
        Self {
            address,
            kind,
            deltas,
            desired,
            premise,
            roused_by,
            owed,
            owed_by: Vec::new(),
            depends_on,
            resent: false,
            past_forgetting: false,
        }
    }

    /// Marks this as a change an entry is sending again, and says whether what
    /// it repeats had passed its forget point.
    ///
    /// Not an argument to `new`, because it is not a part of every change: it
    /// is what state adds to one, as `leave_owed` is.
    #[must_use]
    pub(crate) fn again(mut self, past_forgetting: bool) -> Self {
        self.resent = true;
        self.past_forgetting = past_forgetting;
        self
    }

    /// Whether an earlier run marked this change and never recorded it, so that
    /// it is being sent a second time without anyone knowing what the first did.
    #[must_use]
    pub fn is_resent(&self) -> bool {
        self.resent
    }

    /// Whether the change this repeats had passed its forget point.
    #[must_use]
    pub fn is_past_forgetting(&self) -> bool {
        self.past_forgetting
    }

    /// Everything this resource comes after, in a stable order.
    #[must_use]
    pub fn depends_on(&self) -> &[Address] {
        &self.depends_on
    }

    /// What roused it in an earlier run that did not finish paying.
    #[must_use]
    pub fn owed(&self) -> &[Address] {
        &self.owed
    }

    /// Reacting resources this plan cannot reach, owed a reaction to this change
    /// on their records, in a stable order.
    #[must_use]
    pub fn owed_by(&self) -> &[Address] {
        &self.owed_by
    }

    /// Leaves a reaction to this change owed by `reactors`, which the plan
    /// cannot reach.
    pub(crate) fn leave_owed(&mut self, reactors: Vec<Address>) {
        self.owed_by = reactors;
    }

    /// What changed that this resource reacts to, in a stable order.
    ///
    /// Empty unless something did. A reader is entitled to know why a service is
    /// being bounced before it is, which is the whole of decision 0026's fourth
    /// paragraph.
    #[must_use]
    pub fn roused_by(&self) -> &[Address] {
        &self.roused_by
    }

    /// What the machine looked like when this was decided.
    ///
    /// Empty for a resource nothing was looked at for — a creation has nothing
    /// to have moved — and for an adoption or a reorder, which send a machine
    /// nothing to check it against.
    #[must_use]
    pub fn premise(&self) -> &[nerpa_proto::Premise] {
        &self.premise
    }

    #[dacc_derive::doc_anchor(id = "inv-plan-004-1")]
    /// Every attribute the resource should end up with, in a stable order.
    #[must_use]
    pub fn desired(&self) -> &[(String, Confidence)] {
        &self.desired
    }

    /// Which resource.
    #[must_use]
    pub fn address(&self) -> &Address {
        &self.address
    }

    /// What kind of change.
    #[must_use]
    pub fn kind(&self) -> ChangeKind {
        self.kind
    }

    /// The attributes that differ, in a stable order.
    #[must_use]
    pub fn deltas(&self) -> &[AttributeDelta] {
        &self.deltas
    }

    /// The attribute that forced a replacement, if one did.
    ///
    /// Reported so that plan output can say *why* a resource is being replaced
    /// rather than leaving somebody to infer it from a marker.
    #[must_use]
    pub fn cause(&self) -> Option<&AttributeDelta> {
        self.deltas.iter().find(|delta| delta.forces_replacement)
    }
}

#[dacc_derive::doc_anchor(id = "inv-reach-004-1")]
/// A resource this plan says nothing about because it waits for another that
/// cannot be planned.
///
/// Not a machine that did not answer: its own machine may well have answered, or
/// it may be on none. Listed apart from [`Unreachable`], so that a plan never says
/// a machine was silent when it was not, and names a change held on no machine at
/// all. See `INV-REACH-004`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Waiting {
    address: Address,
    on: Address,
}

impl Waiting {
    pub(crate) fn new(address: Address, on: Address) -> Self {
        Self { address, on }
    }

    /// Which resource waits.
    #[must_use]
    pub fn address(&self) -> &Address {
        &self.address
    }

    /// What it waits for: out of reach itself, or waiting in turn.
    #[must_use]
    pub fn on(&self) -> &Address {
        &self.on
    }
}

/// A machine, or the control domain, that a run left mid-change.
///
/// Its shard holds a mark in flight: a request was written down as going out and
/// no answer was ever written. Whether it arrived, and what it did, went with
/// the run that stopped — so nothing is planned there until a person says the
/// machine is theirs again (`nerpa release`). RFC 0001 §5.8.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Held {
    domain: Option<Target>,
    since: Written,
    changes: usize,
}

impl Held {
    pub(crate) fn new(domain: Option<Target>, since: Written, changes: usize) -> Self {
        Self {
            domain,
            since,
            changes,
        }
    }

    /// Which machine, or `None` for the control domain — which has no name a
    /// machine could also have.
    #[must_use]
    pub fn domain(&self) -> Option<&Target> {
        self.domain.as_ref()
    }

    /// When the change that holds it was written down.
    #[must_use]
    pub fn since(&self) -> Written {
        self.since
    }

    /// How many of this plan's resources it holds.
    #[must_use]
    pub fn changes(&self) -> usize {
        self.changes
    }
}

/// A resource nothing can be said about, because its machine did not answer.
///
/// Not a change, and deliberately not inside a stage: putting it there would let
/// it be iterated as work to do. It carries the machine and the age of the newest
/// thing known about it, and nothing else — a difference computed from stale data
/// looks exactly like one computed from a live read, and there is no honest way
/// to show both. See decision 0020.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Unreachable {
    address: Address,
    node: Target,
    known_for: Option<u64>,
}

impl Unreachable {
    pub(crate) fn new(address: Address, node: Target, known_for: Option<u64>) -> Self {
        Self {
            address,
            node,
            known_for,
        }
    }

    /// Which resource cannot be spoken for.
    #[must_use]
    pub fn address(&self) -> &Address {
        &self.address
    }

    /// The machine that did not answer.
    #[must_use]
    pub fn node(&self) -> &Target {
        &self.node
    }

    /// How many seconds old the newest knowledge of it is, or `None` if there is
    /// none at all.
    #[must_use]
    pub fn known_for(&self) -> Option<u64> {
        self.known_for
    }
}

/// A resource made from something that is no longer declared.
///
/// Reported, never acted on. A running machine's boot disk cannot be swapped
/// underneath it, so an edit to an image reference must not become a proposal to
/// destroy the fleet — but silence would let a fleet drift years behind with
/// nobody told. See decision 0021.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Outdated {
    address: Address,
    attribute: String,
    born_with: Option<Recorded>,
    declared: Confidence,
}

impl Outdated {
    pub(crate) fn new(
        address: Address,
        attribute: String,
        born_with: Option<Recorded>,
        declared: Confidence,
    ) -> Self {
        Self {
            address,
            attribute,
            born_with,
            declared,
        }
    }

    /// Which resource.
    #[must_use]
    pub fn address(&self) -> &Address {
        &self.address
    }

    /// Which attribute it was born with.
    #[must_use]
    pub fn attribute(&self) -> &str {
        &self.attribute
    }

    /// What it was made from, if that is recorded.
    #[must_use]
    pub fn born_with(&self) -> Option<&Recorded> {
        self.born_with.as_ref()
    }

    /// What the configuration says now.
    #[must_use]
    pub fn declared(&self) -> &Confidence {
        &self.declared
    }
}

/// One topological level of the graph, and what happens in it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PlanStage {
    kind: StageKind,
    changes: Vec<Change>,
}

impl PlanStage {
    pub(crate) fn new(kind: StageKind, changes: Vec<Change>) -> Self {
        Self { kind, changes }
    }

    /// What sort of work this stage is made of.
    #[must_use]
    pub fn kind(&self) -> &StageKind {
        &self.kind
    }

    /// The changes, in a stable order.
    #[must_use]
    pub fn changes(&self) -> &[Change] {
        &self.changes
    }
}

/// How much of the world a plan touches.
///
/// Read before the detail rather than after it, because the first question is
/// always how bad this could be — and for an agent it is the only question that
/// can be answered without reading everything.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BlastRadius {
    creates: usize,
    updates: usize,
    replaces: usize,
    deletes: usize,
    reacts: usize,
    adopts: usize,
    reorders: usize,
    unresolved: usize,
    unreachable: usize,
    #[serde(default)]
    waiting: usize,
    #[serde(default)]
    held: usize,
    outdated: usize,
    nodes: BTreeSet<Target>,
}

impl BlastRadius {
    /// Resources that will be made to act because something they react to
    /// changed — a service restarted by a configuration file, usually.
    #[must_use]
    pub fn reacts(&self) -> usize {
        self.reacts
    }

    /// Resources found as declared and not yet recorded, which will be written
    /// down without anything being done to them.
    #[must_use]
    pub fn adopts(&self) -> usize {
        self.adopts
    }

    /// Recorded resources whose edges changed, which will be recorded again
    /// without anything being done to them.
    #[must_use]
    pub fn reorders(&self) -> usize {
        self.reorders
    }

    /// Resources that will come into existence.
    #[must_use]
    pub fn creates(&self) -> usize {
        self.creates
    }

    /// Resources changing in place.
    #[must_use]
    pub fn updates(&self) -> usize {
        self.updates
    }

    /// Resources that will be destroyed and made again.
    #[must_use]
    pub fn replaces(&self) -> usize {
        self.replaces
    }

    /// Resources that will be removed and not put back.
    #[must_use]
    pub fn deletes(&self) -> usize {
        self.deletes
    }

    /// Attribute values that will only be known once something has been created.
    #[must_use]
    pub fn unresolved(&self) -> usize {
        self.unresolved
    }

    /// Every machine this plan touches.
    #[must_use]
    pub fn nodes(&self) -> &BTreeSet<Target> {
        &self.nodes
    }

    /// Resources made from something no longer declared.
    ///
    /// Never work. A number, so that a fleet falling behind is a fact somebody is
    /// handed rather than one they discover.
    #[must_use]
    pub fn outdated(&self) -> usize {
        self.outdated
    }

    /// Resources nothing can be said about.
    ///
    /// A number rather than a sentence, so something automated can act on it —
    /// and the reason a plan may not be signed, because nobody can approve a
    /// change to infrastructure they could not look at.
    #[must_use]
    pub fn unreachable(&self) -> usize {
        self.unreachable
    }

    /// Resources that wait for something that cannot be planned.
    #[must_use]
    pub fn waiting(&self) -> usize {
        self.waiting
    }

    /// Resources on a machine a run left mid-change.
    #[must_use]
    pub fn held(&self) -> usize {
        self.held
    }

    /// Whether anything here cannot be undone.
    ///
    /// A replacement destroys the resource that is there now, and a deletion
    /// removes it for good. Nothing else in a plan does.
    #[must_use]
    pub fn has_irreversible(&self) -> bool {
        self.replaces + self.deletes > 0
    }

    /// How many resources the plan does something about, adoptions and reorders
    /// included.
    #[must_use]
    pub fn total(&self) -> usize {
        self.creates
            + self.updates
            + self.replaces
            + self.deletes
            + self.reacts
            + self.adopts
            + self.reorders
    }
}

#[dacc_derive::doc_anchor(id = "inv-agent-001-2")]
/// Everything a run intends to do, in the order it will do it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct Plan {
    stages: Vec<PlanStage>,
    unreachable: Vec<Unreachable>,
    #[serde(default)]
    waiting: Vec<Waiting>,
    #[serde(default)]
    held: Vec<Held>,
    outdated: Vec<Outdated>,
    blast_radius: BlastRadius,
}

impl Plan {
    pub(crate) fn new(
        stages: Vec<PlanStage>,
        unreachable: Vec<Unreachable>,
        waiting: Vec<Waiting>,
        held: Vec<Held>,
        outdated: Vec<Outdated>,
    ) -> Self {
        let mut blast_radius = BlastRadius {
            unreachable: unreachable.len(),
            waiting: waiting.len(),
            held: held.iter().map(Held::changes).sum(),
            outdated: outdated.len(),
            ..Default::default()
        };
        for change in stages.iter().flat_map(PlanStage::changes) {
            match change.kind() {
                ChangeKind::Create => blast_radius.creates += 1,
                ChangeKind::Update => blast_radius.updates += 1,
                ChangeKind::Replace => blast_radius.replaces += 1,
                ChangeKind::Delete => blast_radius.deletes += 1,
                ChangeKind::React => blast_radius.reacts += 1,
                ChangeKind::Adopt => blast_radius.adopts += 1,
                ChangeKind::Reorder => blast_radius.reorders += 1,
            }
            blast_radius.unresolved += change
                .desired()
                .iter()
                .filter(|(_, value)| matches!(value, Confidence::Unknown(_)))
                .count();
            if let Some(target) = change.address().target() {
                blast_radius.nodes.insert(target.clone());
            }
        }
        Self {
            stages,
            unreachable,
            waiting,
            held,
            outdated,
            blast_radius,
        }
    }

    /// Resources made from something no longer declared, in a stable order.
    ///
    /// Not work, and not a reason to refuse anything. Moving a fleet onto a new
    /// image is a deliberate act — replacement on demand, or two slots side by
    /// side until the new one has shown itself.
    #[must_use]
    pub fn outdated(&self) -> &[Outdated] {
        &self.outdated
    }

    /// The resources nothing can be said about, in a stable order.
    #[must_use]
    pub fn unreachable(&self) -> &[Unreachable] {
        &self.unreachable
    }

    /// The resources that wait for something that cannot be planned, and what
    /// for, in a stable order.
    #[must_use]
    pub fn waiting(&self) -> &[Waiting] {
        &self.waiting
    }

    /// The machines a run left mid-change, in a stable order.
    #[must_use]
    pub fn held(&self) -> &[Held] {
        &self.held
    }

    #[dacc_derive::doc_anchor(id = "inv-reach-003-2")]
    /// Whether everything the configuration declares could be looked at.
    ///
    /// A plan that is not complete cannot be signed: it holds an age, an age
    /// moves every second, and a hash over it never settles. That is the right
    /// answer rather than an inconvenience — see decision 0020.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.unreachable.is_empty() && self.waiting.is_empty() && self.held.is_empty()
    }

    /// The stages, in execution order. Stages with nothing to do are not here.
    #[must_use]
    pub fn stages(&self) -> &[PlanStage] {
        &self.stages
    }

    /// How much of the world this touches.
    #[must_use]
    pub fn blast_radius(&self) -> &BlastRadius {
        &self.blast_radius
    }

    /// Whether the world already matches the configuration.
    ///
    /// The property a convergent tool exists to have: applying a plan and
    /// planning again must produce one of these.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.stages.is_empty()
    }

    /// Every change, in execution order, ignoring stage boundaries.
    pub fn changes(&self) -> impl Iterator<Item = &Change> {
        self.stages.iter().flat_map(PlanStage::changes)
    }
}

/// The bytes a plan digest is taken over start with this, so that a digest of a
/// plan can never collide with a digest of anything else and so that changing
/// how a plan is encoded changes every digest rather than some of them.
const PLAN_DIGEST_DOMAIN: &[u8] = b"nerpa-plan-digest-1";

/// One number standing for everything a plan intends to do.
///
/// What it covers is exactly the intent: the stages, and every change in them.
/// Not what the plan could not see — a plan carrying an age cannot be hashed at
/// all — and not what it merely reports. Those belong to the premise, which
/// covers what the plan was computed *from*. Between the two the whole plan is
/// accounted for, and neither is doing the other's job.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlanDigest([u8; crate::secret::LENGTH]);

impl PlanDigest {
    /// Written as hexadecimal.
    #[must_use]
    pub fn to_hex(&self) -> String {
        crate::secret::to_hex(&self.0)
    }

    /// Read back from hexadecimal.
    #[must_use]
    pub fn from_hex(text: &str) -> Option<Self> {
        crate::secret::from_hex(text).map(Self)
    }
}

impl fmt::Debug for PlanDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PlanDigest({})", self.to_hex())
    }
}

impl fmt::Display for PlanDigest {
    /// The first twelve characters, which is what a person compares by eye.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.to_hex().get(..12).unwrap_or_default())
    }
}

impl serde::Serialize for PlanDigest {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_hex())
    }
}

/// Why a plan has no digest.
///
/// Both counts are named, because a plan can be incomplete for either reason, and
/// one that only waits used to say it could not look at none of its machines.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
#[error(
    "a plan has no digest while it could not look at {machines} of its resources, \
     {waiting} of them wait for those, and {held} are on a machine a run left \
     mid-change: it carries how old that knowledge is, an age moves every second, \
     and a number over it would never settle"
)]
pub struct Unhashable {
    /// How many resources their own machine could not be asked about.
    pub machines: usize,
    /// How many wait for something that cannot be planned.
    pub waiting: usize,
    /// How many are on a machine a run left mid-change.
    pub held: usize,
}

impl Plan {
    /// One number standing for everything this plan intends to do.
    ///
    /// # Errors
    ///
    /// Returns [`Unhashable`] if any machine could not be looked at. Such a plan
    /// is incomplete by `INV-REACH-003` and says so; a digest over it would move
    /// every second, which is worse than not having one.
    #[dacc_derive::doc_anchor(id = "inv-plan-006-1")]
    pub fn digest(&self) -> Result<PlanDigest, Unhashable> {
        if !self.is_complete() {
            return Err(Unhashable {
                machines: self.unreachable.len(),
                waiting: self.waiting.len(),
                held: self.blast_radius.held,
            });
        }

        let mut out = Vec::from(PLAN_DIGEST_DOMAIN);
        out.extend_from_slice(&value::length(self.stages.len()));
        for stage in &self.stages {
            encode_stage(stage, &mut out);
        }
        Ok(PlanDigest(*blake3::hash(&out).as_bytes()))
    }
}

/// Every kind carries a distinct tag and every variable-length part carries its
/// length, so no two plans can encode to the same bytes.
///
/// Deliberately without a catch-all arm. These enums are `#[non_exhaustive]` for
/// the world outside this crate and exhaustive inside it, so adding a variant
/// stops the build here — which is the right place to be asked how it hashes,
/// rather than having it silently fold into a default and two different plans
/// share a number.
fn encode_stage(stage: &PlanStage, out: &mut Vec<u8>) {
    match stage.kind() {
        StageKind::Control => out.push(1),
        StageKind::Target(nodes) => {
            out.push(2);
            out.extend_from_slice(&value::length(nodes.len()));
            for node in nodes {
                value::text(&node.to_string(), out);
            }
        }
        StageKind::Mixed => out.push(3),
    }

    out.extend_from_slice(&value::length(stage.changes().len()));
    for change in stage.changes() {
        value::text(&change.address().to_string(), out);
        out.push(match change.kind() {
            ChangeKind::Create => 1,
            ChangeKind::Update => 2,
            ChangeKind::Replace => 3,
            ChangeKind::Delete => 4,
            ChangeKind::React => 5,
            ChangeKind::Adopt => 6,
            ChangeKind::Reorder => 7,
        });

        // Why, not only what. Two plans that restart the same service for
        // different reasons are two different plans, and a person approving one
        // of them is approving the reason as well as the act.
        out.extend_from_slice(&value::length(change.roused_by().len()));
        for cause in change.roused_by() {
            value::text(&cause.to_string(), out);
        }

        // A debt carried in is part of what this plan intends to do, and a plan
        // paying one is not the same plan as one that has nothing owed.
        out.extend_from_slice(&value::length(change.owed().len()));
        for cause in change.owed() {
            value::text(&cause.to_string(), out);
        }

        // A debt left on the record of something this plan cannot reach. A plan
        // writing one promises a reaction later, and approving it approves that
        // promise.
        out.extend_from_slice(&value::length(change.owed_by().len()));
        for reactor in change.owed_by() {
            value::text(&reactor.to_string(), out);
        }

        // What it comes after, which is what state will remember to order its
        // removal by. A plan recording different edges removes things in a
        // different order later, so the edges are part of what it intends.
        out.extend_from_slice(&value::length(change.depends_on().len()));
        for dependency in change.depends_on() {
            value::text(&dependency.to_string(), out);
        }

        // Sent again, and how far the change it repeats had got. A plan that
        // repeats a change nobody knows the outcome of is not the plan that
        // would have made that change afresh, and a person approving the one is
        // not approving the other.
        out.push(u8::from(change.is_resent()));
        out.push(u8::from(change.is_past_forgetting()));

        out.extend_from_slice(&value::length(change.desired().len()));
        for (name, wanted) in change.desired() {
            value::text(name, out);
            encode_confidence(wanted, out);
        }

        out.extend_from_slice(&value::length(change.deltas().len()));
        for delta in change.deltas() {
            value::text(delta.name(), out);
            match delta.before() {
                None => out.push(0),
                Some(Recorded::Plain(value)) => {
                    out.push(1);
                    value::encode(value, out);
                }
                Some(Recorded::Secret(digest)) => {
                    out.push(2);
                    out.extend_from_slice(digest.raw());
                }
            }
            encode_confidence(delta.after(), out);
            out.push(u8::from(delta.forces_replacement()));
        }
    }
}

fn encode_confidence(confidence: &Confidence, out: &mut Vec<u8>) {
    match confidence {
        Confidence::Known(value) => {
            out.push(1);
            value::encode(value, out);
        }
        Confidence::Unknown(reference) => {
            out.push(2);
            // Two pieces, each with its own length, rather than the printed
            // form. Printing joins them with a separator either piece may
            // itself contain — a resource name may hold a dot — so
            // `os:file.a.b`'s `c` and `os:file.a`'s `b.c` print alike, and a
            // number over the printed form is a number two different plans
            // share. See `two_references_that_differ_only_in_where_the_dot_falls_do_not_collide`.
            value::text(&reference.source().to_string(), out);
            value::text(reference.attribute(), out);
        }
        Confidence::Secret(sealed) => {
            out.push(3);
            encode_source(sealed.source(), out);
            out.extend_from_slice(sealed.digest().raw());
        }
        Confidence::Bounded(choice) => {
            out.push(4);
            value::text(choice.fact().node().as_str(), out);
            value::text(choice.fact().name(), out);
            out.extend_from_slice(&value::length(choice.options().len()));
            for (said, value) in choice.options() {
                value::text(said, out);
                value::encode(value, out);
            }
        }
    }
}

/// Where a secret comes from, piece by piece.
///
/// Not the printed form: that spells a version with `@` and a store path may
/// contain one, so `vault:a@b` is both "path `a@b`, current version" and "path
/// `a`, version `b`". Two different places in a store, one string — and if the
/// two happen to hold the same value, one plan number. Which secret a plan
/// reads is part of what somebody approves.
fn encode_source(source: &Source, out: &mut Vec<u8>) {
    match source {
        Source::Stored {
            provider,
            path,
            version,
        } => {
            out.push(1);
            value::text(provider, out);
            value::text(path, out);
            match version.as_deref() {
                Some(version) => {
                    out.push(1);
                    value::text(version, out);
                }
                None => out.push(0),
            }
        }
    }
}

/// Written out rather than derived so that the digest travels with the plan.
///
/// It is computed rather than stored, so reading a plan back and recomputing it
/// is the check that the document was not edited on the way — which is the whole
/// point of a machine having the number at all. `serde` ignores fields it does
/// not know, so the derived `Deserialize` reads this document unchanged.
impl serde::Serialize for Plan {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap as _;
        let mut map = serializer.serialize_map(Some(7))?;
        map.serialize_entry("stages", &self.stages)?;
        map.serialize_entry("unreachable", &self.unreachable)?;
        map.serialize_entry("waiting", &self.waiting)?;
        map.serialize_entry("held", &self.held)?;
        map.serialize_entry("outdated", &self.outdated)?;
        map.serialize_entry("blast_radius", &self.blast_radius)?;
        // `null` rather than absent when there is none: a reader that finds no
        // key cannot tell an old document from an unhashable plan, and those
        // mean very different things.
        map.serialize_entry("digest", &self.digest().ok())?;
        map.end()
    }
}

#[cfg(test)]
mod tests {
    use super::{Change, ChangeKind, PlanStage, StageKind, encode_stage};
    use crate::address::address;

    /// Two changes alike in every other way: one being sent again, one not.
    ///
    /// No plan can hold that pair — a change sent again is one whose reading
    /// matched, and one worked out afresh is one whose reading did not, so any
    /// two the planner builds differ in their deltas as well. Which is exactly
    /// why this is here: nothing comparing whole plans can say the bit is in
    /// the number rather than the difference that came with it.
    #[test]
    fn whether_a_change_is_sent_again_is_in_the_number() {
        let plain = || {
            Change::new(
                address("os:file.motd@web-01"),
                ChangeKind::Update,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
        };
        let encoded = |change: Change| {
            let mut out = Vec::new();
            encode_stage(&PlanStage::new(StageKind::Mixed, vec![change]), &mut out);
            out
        };

        assert_ne!(
            encoded(plain()),
            encoded(plain().again(false)),
            "a repeat of a change nobody knows the outcome of is not that change"
        );
        assert_ne!(
            encoded(plain().again(false)),
            encoded(plain().again(true)),
            "nor is repeating half of a replacement the same as repeating all of it"
        );
    }
}
