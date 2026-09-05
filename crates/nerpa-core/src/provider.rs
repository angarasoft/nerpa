//! The boundary between the model and anything that touches the world.
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
pub trait Provider {
    /// The name this provider answers to in an address.
    fn name(&self) -> &ProviderName;

    /// What this provider refuses to change in place.
    fn schema(&self) -> Schema;

    /// What the resource looks like now, or `None` if it does not exist.
    ///
    /// `declared` is needed because an address does not always say where in the
    /// world to look: a file is found by its `path`, and that lives in the
    /// declaration. Only settled values are passed — anything still waiting on
    /// another resource is left out, which is correct, because a resource whose
    /// very location depends on something that has not been created cannot be
    /// there yet.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] if the resource could not be read at all. Not
    /// existing is an answer, not a failure.
    fn read(
        &self,
        address: &Address,
        declared: &Attributes,
    ) -> Result<Option<Attributes>, ProviderError>;

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
    fn delete(&self, address: &Address, recorded: &Attributes) -> Result<(), ProviderError>;

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
