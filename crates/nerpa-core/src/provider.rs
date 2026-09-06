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
//! who are not reading this ([0015](../../../docs/decisions)).

use std::collections::BTreeMap;

use crate::address::{Address, Provider as ProviderName, Target};
use crate::plan::ChangeKind;
use crate::schema::Schema;
use crate::value::Value;

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

/// Reads and changes resources of the kinds it owns.
// ENFORCES: INV-PROVIDER-001
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
        reach: &dyn Reach,
    ) -> Result<BTreeMap<Address, Attributes>, ProviderError>;

    /// Makes the resource match `attributes`, and reports what it became.
    ///
    /// Every value here is settled: whatever was unresolved at plan time has been
    /// resolved from what earlier stages produced.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] if the declaration is unusable or the world
    /// refuses the change.
    fn apply(
        &self,
        address: &Address,
        kind: ChangeKind,
        attributes: &Attributes,
        reach: &dyn Reach,
    ) -> Result<Attributes, ProviderError>;

    /// Removes the resource `recorded` describes.
    ///
    /// Takes what state remembers rather than what the configuration says,
    /// because by the time something is being removed the configuration that
    /// described it is gone.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] if the resource could not be removed. Already
    /// being gone is not a failure.
    fn delete(
        &self,
        address: &Address,
        recorded: &Attributes,
        reach: &dyn Reach,
    ) -> Result<(), ProviderError>;

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
/// ([0032](../../../docs/decisions)); this is the seam. A provider composes what
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
