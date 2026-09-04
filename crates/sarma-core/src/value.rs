//! Attribute values, and the fact that some of them do not exist yet.
//!
//! Deliberately small. This is the vocabulary a provider schema is expressed in
//! and a plan diff is printed from, not a general purpose data model; anything
//! richer belongs in the provider that understands it.

use std::collections::BTreeMap;
use std::fmt;

/// A settled attribute value.
///
/// Maps are ordered because a plan is hashed: two runs that produce the same
/// attributes must produce the same bytes, and hash map iteration order would
/// make that untrue on alternate Tuesdays.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
/// Non-exhaustive: provider schemas will want types this does not have yet, and
/// adding one should not be a breaking change for everybody matching on it.
#[non_exhaustive]
pub enum Value {
    /// A string.
    Text(String),
    /// A signed integer. There is no floating point on purpose: a plan is
    /// hashed, and two bit patterns that compare equal would not hash equal.
    Integer(i64),
    /// A boolean.
    Boolean(bool),
    /// An ordered sequence.
    List(Vec<Value>),
    /// An ordered mapping. Ordered because a plan is hashed.
    Map(BTreeMap<String, Value>),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Text(text) => write!(f, "{text:?}"),
            Value::Integer(number) => write!(f, "{number}"),
            Value::Boolean(flag) => write!(f, "{flag}"),
            Value::List(items) => {
                f.write_str("[")?;
                for (position, item) in items.iter().enumerate() {
                    if position > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{item}")?;
                }
                f.write_str("]")
            }
            Value::Map(entries) => {
                f.write_str("{")?;
                for (position, (key, item)) in entries.iter().enumerate() {
                    if position > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{key}: {item}")?;
                }
                f.write_str("}")
            }
        }
    }
}

impl From<&str> for Value {
    fn from(text: &str) -> Self {
        Value::Text(text.to_owned())
    }
}

impl From<String> for Value {
    fn from(text: String) -> Self {
        Value::Text(text)
    }
}

impl From<i64> for Value {
    fn from(number: i64) -> Self {
        Value::Integer(number)
    }
}

impl From<bool> for Value {
    fn from(flag: bool) -> Self {
        Value::Boolean(flag)
    }
}
