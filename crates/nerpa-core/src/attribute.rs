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

use std::collections::BTreeMap;

use crate::address::{Address, Target};
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

/// Something read from a machine rather than declared.
///
/// A fact is not a resource attribute, so it is not a [`Reference`]: nothing
/// created it and nothing depends on it existing. It is what the machine turned
/// out to be.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct Fact {
    node: Target,
    name: String,
}

impl Fact {
    /// Names a fact of a machine.
    pub fn of(node: Target, name: impl Into<String>) -> Self {
        Self {
            node,
            name: name.into(),
        }
    }

    /// Whose fact this is.
    #[must_use]
    pub fn node(&self) -> &Target {
        &self.node
    }

    /// Which fact.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl fmt::Display for Fact {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}'s {}", self.node, self.name)
    }
}

/// A value chosen from a closed set by a fact.
///
/// The mechanism decision 0029 settles. Every branch is written down, so the
/// graph is fully known — one resource, one edge, one place in the order — and
/// only the value is open, within a set the plan can print. That is more useful
/// than "unknown after apply", which names no possibilities at all.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Choice {
    fact: Fact,
    options: BTreeMap<String, Value>,
}

impl Choice {
    /// Chooses among these by that fact.
    #[must_use]
    pub fn among(fact: Fact, options: BTreeMap<String, Value>) -> Self {
        Self { fact, options }
    }

    /// What decides.
    #[must_use]
    pub fn fact(&self) -> &Fact {
        &self.fact
    }

    /// Everything it could be, by the fact value that would pick it.
    #[must_use]
    pub fn options(&self) -> &BTreeMap<String, Value> {
        &self.options
    }

    /// What this would be if the fact turned out to say that.
    #[must_use]
    pub fn given(&self, said: &str) -> Option<&Value> {
        self.options.get(said)
    }
}

impl fmt::Display for Choice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "one of ")?;
        for (position, value) in self.options.values().enumerate() {
            if position > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{value}")?;
        }
        write!(f, ", by {}", self.fact)
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
    /// Chosen from a closed set by something read from the machine.
    ///
    /// A fact may decide what an attribute *says* and never whether a resource
    /// exists. That is the whole boundary, and writing it here rather than
    /// leaving it to an error message is the point of decision 0029.
    Chosen(Choice),
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
    #[dacc_derive::doc_anchor(id = "inv-secret-001")]
    pub fn known(&self) -> Option<&Value> {
        match self {
            Attribute::Known(value) => Some(value),
            Attribute::Unresolved(_) | Attribute::Secret(_) | Attribute::Chosen(_) => None,
        }
    }

    /// Where the secret behind this attribute is kept, if it is a secret.
    pub fn secret(&self) -> Option<&Source> {
        match self {
            Attribute::Secret(source) => Some(source),
            Attribute::Known(_) | Attribute::Unresolved(_) | Attribute::Chosen(_) => None,
        }
    }

    /// Whether this attribute holds something that must not be written down.
    pub fn is_secret(&self) -> bool {
        matches!(self, Attribute::Secret(_))
    }

    /// What this attribute is waiting on, if it is waiting.
    #[dacc_derive::doc_anchor(id = "inv-graph-003-1")]
    pub fn reference(&self) -> Option<&Reference> {
        match self {
            Attribute::Known(_) | Attribute::Secret(_) | Attribute::Chosen(_) => None,
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
            Attribute::Chosen(choice) => write!(f, "{choice}"),
        }
    }
}

impl Attribute {
    /// What a fact would have to say for this attribute to be settled, if it is
    /// waiting on one.
    #[must_use]
    pub fn chosen(&self) -> Option<&Choice> {
        match self {
            Attribute::Chosen(choice) => Some(choice),
            Attribute::Known(_) | Attribute::Unresolved(_) | Attribute::Secret(_) => None,
        }
    }
}

impl From<Choice> for Attribute {
    fn from(choice: Choice) -> Self {
        Attribute::Chosen(choice)
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
