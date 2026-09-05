//! What an attribute holds, including values that do not exist yet.
//!
//! Configuration routinely names something that will only be known once
//! something else has been created — an address the cloud allocates, an
//! identifier minted on creation. Those are carried as a [`Reference`] to the
//! resource that will supply them, never as a placeholder value.
//!
//! # Why this is not part of `Value`
//!
//! [`Value`] describes settled data, and it is also what
//! observed state is made of. An observed value cannot be unknown — a machine
//! either has a package installed or it does not, and there is no third reading.
//! Folding both into one type would make that nonsense representable, and
//! something would eventually represent it.
//!
//! So the split runs along what the data means: [`Value`] for anything that has
//! been settled, `Attribute` for what a declaration says, which may still be
//! waiting on the world.

use std::fmt;

use crate::address::Address;
use crate::value::Value;

/// Where a value will come from once it exists.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct Reference {
    source: Address,
    attribute: String,
}

impl Reference {
    /// Names the resource and attribute that will supply this value.
    pub fn to(source: Address, attribute: impl Into<String>) -> Self {
        Self {
            source,
            attribute: attribute.into(),
        }
    }

    /// The resource that will supply it.
    pub fn source(&self) -> &Address {
        &self.source
    }

    /// The attribute of that resource to read.
    pub fn attribute(&self) -> &str {
        &self.attribute
    }
}

impl fmt::Display for Reference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.source, self.attribute)
    }
}

/// What a declaration says about one attribute.
///
/// Non-exhaustive: a third reading is already in the roadmap. A secret is
/// neither settled data nor a reference to another resource — it is a reference
/// to a secret store, and state is required to hold that rather than the value
/// behind it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Attribute {
    /// Settled, and available now.
    Known(Value),
    /// Not settled. Will be read from another resource once that exists.
    Unresolved(Reference),
}

impl Attribute {
    /// The value, if there is one yet.
    pub fn known(&self) -> Option<&Value> {
        match self {
            Attribute::Known(value) => Some(value),
            Attribute::Unresolved(_) => None,
        }
    }

    /// What this attribute is waiting on, if it is waiting.
    // ENFORCES: INV-GRAPH-003
    pub fn reference(&self) -> Option<&Reference> {
        match self {
            Attribute::Known(_) => None,
            Attribute::Unresolved(reference) => Some(reference),
        }
    }
}

impl fmt::Display for Attribute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Attribute::Known(value) => write!(f, "{value}"),
            Attribute::Unresolved(reference) => write!(f, "unknown until apply, from {reference}"),
        }
    }
}

impl From<Reference> for Attribute {
    fn from(reference: Reference) -> Self {
        Attribute::Unresolved(reference)
    }
}

impl<T: Into<Value>> From<T> for Attribute {
    fn from(value: T) -> Self {
        Attribute::Known(value.into())
    }
}
