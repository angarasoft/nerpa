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
use crate::secret::Source;
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
/// Non-exhaustive: provider schemas will want readings this does not have yet.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Attribute {
    /// Settled, and available now.
    Known(Value),
    /// Not settled. Will be read from another resource once that exists.
    Unresolved(Reference),
    /// Settled somewhere else, and deliberately not brought here.
    ///
    /// The declaration names a store and a path; the value is fetched when a
    /// provider needs it and is never written into state or into a plan. See
    /// decision 0027.
    Secret(Source),
}

impl Attribute {
    /// The value, if there is one yet.
    ///
    /// A secret answers `None` here even though its value exists: it exists
    /// somewhere else, and this is the accessor every part of the engine that
    /// writes things down goes through.
    pub fn known(&self) -> Option<&Value> {
        match self {
            Attribute::Known(value) => Some(value),
            Attribute::Unresolved(_) | Attribute::Secret(_) => None,
        }
    }

    /// Where the secret behind this attribute is kept, if it is a secret.
    // ENFORCES: INV-SECRET-001
    pub fn secret(&self) -> Option<&Source> {
        match self {
            Attribute::Secret(source) => Some(source),
            Attribute::Known(_) | Attribute::Unresolved(_) => None,
        }
    }

    /// Whether this attribute holds something that must not be written down.
    pub fn is_secret(&self) -> bool {
        matches!(self, Attribute::Secret(_))
    }

    /// What this attribute is waiting on, if it is waiting.
    // ENFORCES: INV-GRAPH-003
    pub fn reference(&self) -> Option<&Reference> {
        match self {
            Attribute::Known(_) | Attribute::Secret(_) => None,
            Attribute::Unresolved(reference) => Some(reference),
        }
    }
}

impl fmt::Display for Attribute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Attribute::Known(value) => write!(f, "{value}"),
            Attribute::Unresolved(reference) => write!(f, "unknown until apply, from {reference}"),
            Attribute::Secret(source) => write!(f, "(secret from {source})"),
        }
    }
}

impl From<Source> for Attribute {
    fn from(source: Source) -> Self {
        Attribute::Secret(source)
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
