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
use crate::attribute::Reference;
use crate::graph::StageKind;
use std::fmt;

use crate::secret::{Recorded, Sealed};
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
}

impl ChangeKind {
    /// Whether the resource that is there now survives this.
    #[must_use]
    pub fn is_reversible(self) -> bool {
        matches!(self, ChangeKind::Create | ChangeKind::Update)
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
}

impl Change {
    pub(crate) fn new(
        address: Address,
        kind: ChangeKind,
        deltas: Vec<AttributeDelta>,
        desired: Vec<(String, Confidence)>,
    ) -> Self {
        Self {
            address,
            kind,
            deltas,
            desired,
        }
    }

    // ENFORCES: INV-PLAN-004
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
    unresolved: usize,
    unreachable: usize,
    outdated: usize,
    nodes: BTreeSet<Target>,
}

impl BlastRadius {
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

    /// Whether anything here cannot be undone.
    ///
    /// A replacement destroys the resource that is there now, and a deletion
    /// removes it for good. Nothing else in a plan does.
    #[must_use]
    pub fn has_irreversible(&self) -> bool {
        self.replaces + self.deletes > 0
    }

    /// How many resources change at all.
    #[must_use]
    pub fn total(&self) -> usize {
        self.creates + self.updates + self.replaces + self.deletes
    }
}

// ENFORCES: INV-AGENT-001
/// Everything a run intends to do, in the order it will do it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct Plan {
    stages: Vec<PlanStage>,
    unreachable: Vec<Unreachable>,
    outdated: Vec<Outdated>,
    blast_radius: BlastRadius,
}

impl Plan {
    pub(crate) fn new(
        stages: Vec<PlanStage>,
        unreachable: Vec<Unreachable>,
        outdated: Vec<Outdated>,
    ) -> Self {
        let mut blast_radius = BlastRadius {
            unreachable: unreachable.len(),
            outdated: outdated.len(),
            ..Default::default()
        };
        for change in stages.iter().flat_map(PlanStage::changes) {
            match change.kind() {
                ChangeKind::Create => blast_radius.creates += 1,
                ChangeKind::Update => blast_radius.updates += 1,
                ChangeKind::Replace => blast_radius.replaces += 1,
                ChangeKind::Delete => blast_radius.deletes += 1,
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

    // ENFORCES: INV-REACH-003
    /// Whether everything the configuration declares could be looked at.
    ///
    /// A plan that is not complete cannot be signed: it holds an age, an age
    /// moves every second, and a hash over it never settles. That is the right
    /// answer rather than an inconvenience — see decision 0020.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.unreachable.is_empty()
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
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
#[error(
    "a plan that could not look at {machines} of its machines has no digest: it \
     carries how old that knowledge is, an age moves every second, and a number \
     over it would never settle"
)]
pub struct Unhashable {
    /// How many machines did not answer.
    pub machines: usize,
}

impl Plan {
    /// One number standing for everything this plan intends to do.
    ///
    /// # Errors
    ///
    /// Returns [`Unhashable`] if any machine could not be looked at. Such a plan
    /// is incomplete by `INV-REACH-003` and says so; a digest over it would move
    /// every second, which is worse than not having one.
    // ENFORCES: INV-PLAN-006
    pub fn digest(&self) -> Result<PlanDigest, Unhashable> {
        if !self.unreachable.is_empty() {
            return Err(Unhashable {
                machines: self.unreachable.len(),
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
        });

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
            value::text(&reference.to_string(), out);
        }
        Confidence::Secret(sealed) => {
            out.push(3);
            value::text(&sealed.source().to_string(), out);
            out.extend_from_slice(sealed.digest().raw());
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
        let mut map = serializer.serialize_map(Some(5))?;
        map.serialize_entry("stages", &self.stages)?;
        map.serialize_entry("unreachable", &self.unreachable)?;
        map.serialize_entry("outdated", &self.outdated)?;
        map.serialize_entry("blast_radius", &self.blast_radius)?;
        // `null` rather than absent when there is none: a reader that finds no
        // key cannot tell an old document from an unhashable plan, and those
        // mean very different things.
        map.serialize_entry("digest", &self.digest().ok())?;
        map.end()
    }
}
