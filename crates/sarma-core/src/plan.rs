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

use std::collections::BTreeSet;

use crate::address::{Address, Target};
use crate::attribute::Reference;
use crate::graph::StageKind;
use crate::value::Value;

/// What is about to happen to one resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum ChangeKind {
    /// It does not exist, and will.
    Create,
    /// It exists, and some attribute changes in place.
    Update,
    /// It exists, and an attribute that cannot change in place has changed, so
    /// it goes and a new one takes its address.
    Replace,
}

impl ChangeKind {
    /// Whether the old resource survives this.
    #[must_use]
    pub fn is_reversible(self) -> bool {
        !matches!(self, ChangeKind::Replace)
    }
}

/// A value the plan is about to write, and whether it is known yet.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Confidence {
    /// Settled: this exact value will be written.
    Known(Value),
    /// Not settled at plan time, and will be read from another resource once
    /// that resource exists.
    Unknown(Reference),
}

/// One attribute changing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributeDelta {
    name: String,
    before: Option<Value>,
    after: Confidence,
    forces_replacement: bool,
}

impl AttributeDelta {
    pub(crate) fn new(
        name: String,
        before: Option<Value>,
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
    pub fn before(&self) -> Option<&Value> {
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    address: Address,
    kind: ChangeKind,
    deltas: Vec<AttributeDelta>,
}

impl Change {
    pub(crate) fn new(address: Address, kind: ChangeKind, deltas: Vec<AttributeDelta>) -> Self {
        Self {
            address,
            kind,
            deltas,
        }
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

/// One topological level of the graph, and what happens in it.
#[derive(Debug, Clone, PartialEq, Eq)]
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
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BlastRadius {
    creates: usize,
    updates: usize,
    replaces: usize,
    unresolved: usize,
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

    /// Whether anything here cannot be undone.
    ///
    /// A replacement destroys the resource that is there now. Nothing else in a
    /// plan does.
    #[must_use]
    pub fn has_irreversible(&self) -> bool {
        self.replaces > 0
    }

    /// How many resources change at all.
    #[must_use]
    pub fn total(&self) -> usize {
        self.creates + self.updates + self.replaces
    }
}

/// Everything a run intends to do, in the order it will do it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    stages: Vec<PlanStage>,
    blast_radius: BlastRadius,
}

impl Plan {
    pub(crate) fn new(stages: Vec<PlanStage>) -> Self {
        let mut blast_radius = BlastRadius::default();
        for change in stages.iter().flat_map(PlanStage::changes) {
            match change.kind() {
                ChangeKind::Create => blast_radius.creates += 1,
                ChangeKind::Update => blast_radius.updates += 1,
                ChangeKind::Replace => blast_radius.replaces += 1,
            }
            blast_radius.unresolved += change
                .deltas()
                .iter()
                .filter(|delta| matches!(delta.after(), Confidence::Unknown(_)))
                .count();
            if let Some(target) = change.address().target() {
                blast_radius.nodes.insert(target.clone());
            }
        }
        Self {
            stages,
            blast_radius,
        }
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
