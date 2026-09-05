//! Attribute values, and the fact that some of them do not exist yet.
//!
//! Deliberately small. This is the vocabulary a provider schema is expressed in
//! and a plan diff is printed from, not a general purpose data model; anything
//! richer belongs in the provider that understands it.

use std::collections::BTreeMap;
use std::fmt;

use base64::Engine as _;

/// The key a binary value is written under in a human-readable format.
///
/// Reserved: a map with this as its only key cannot be written down, because
/// that shape is how bytes are recorded and reading it back would give something
/// other than what was written. Refused at the point of writing, so no reader
/// ever meets the ambiguity.
const BYTES_KEY: &str = "__bytes__";

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
    /// Bytes that are not text.
    ///
    /// A certificate, a keytab, a font, a configuration file in an encoding that
    /// is not UTF-8: each is a thing somebody manages, and each was outside this
    /// model entirely. Build one with [`Value::bytes`], which puts anything that
    /// *is* valid UTF-8 into [`Value::Text`] instead — one sequence of bytes then
    /// has one representation, which is what lets a plan be hashed and a second
    /// run be empty.
    Bytes(Vec<u8>),
}

impl Value {
    /// Bytes, as text if they are text.
    ///
    /// The canonical form. Without it a file holding `hello` could be either
    /// variant depending on which side of the model built it, the two would
    /// compare unequal, and a run would never converge.
    #[must_use]
    pub fn bytes(bytes: Vec<u8>) -> Self {
        match String::from_utf8(bytes) {
            Ok(text) => Value::Text(text),
            Err(error) => Value::Bytes(error.into_bytes()),
        }
    }

    /// What this holds as bytes, for anything that has to write it out.
    #[must_use]
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Value::Text(text) => Some(text.as_bytes()),
            Value::Bytes(bytes) => Some(bytes),
            _ => None,
        }
    }
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
            // The length alone would read as "no change" for two different files
            // of the same size, so the first few bytes come too.
            Value::Bytes(bytes) => {
                write!(f, "<{} bytes: ", bytes.len())?;
                for byte in bytes.iter().take(8) {
                    write!(f, "{byte:02x}")?;
                }
                if bytes.len() > 8 {
                    f.write_str("…")?;
                }
                f.write_str(">")
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

// ENFORCES: INV-VALUE-001
/// A canonical encoding of a value, for anything that has to hash one.
///
/// Written out rather than borrowed from `serde` because a digest must depend on
/// exactly one encoding for ever, and a serialisation format is free to change
/// how it spells things. Every variant carries a distinct tag and every
/// variable-length part carries its length, so no two different values can
/// encode to the same bytes.
pub(crate) fn encode(value: &Value, out: &mut Vec<u8>) {
    match value {
        Value::Text(text) => {
            out.push(1);
            out.extend_from_slice(&length(text.len()));
            out.extend_from_slice(text.as_bytes());
        }
        Value::Integer(number) => {
            out.push(2);
            out.extend_from_slice(&number.to_le_bytes());
        }
        Value::Boolean(flag) => {
            out.push(3);
            out.push(u8::from(*flag));
        }
        Value::List(items) => {
            out.push(4);
            out.extend_from_slice(&length(items.len()));
            for item in items {
                encode(item, out);
            }
        }
        Value::Bytes(bytes) => {
            out.push(6);
            out.extend_from_slice(&length(bytes.len()));
            out.extend_from_slice(bytes);
        }
        Value::Map(entries) => {
            out.push(5);
            out.extend_from_slice(&length(entries.len()));
            for (key, item) in entries {
                out.extend_from_slice(&length(key.len()));
                out.extend_from_slice(key.as_bytes());
                encode(item, out);
            }
        }
    }
}

/// A length as eight canonical bytes.
///
/// `usize` is never wider than `u64` on any target this builds for, so the
/// fallback is unreachable; it is written rather than unwrapped because a digest
/// is not a place to put a panic.
pub(crate) fn length(count: usize) -> [u8; 8] {
    u64::try_from(count).unwrap_or(u64::MAX).to_le_bytes()
}

/// Text with its length in front, so that two pieces of it cannot run together.
pub(crate) fn text(value: &str, out: &mut Vec<u8>) {
    out.extend_from_slice(&length(value.len()));
    out.extend_from_slice(value.as_bytes());
}

/// Whether a map would be read back as bytes rather than as itself.
fn is_binary_shaped(entries: &BTreeMap<String, Value>) -> bool {
    let mut pairs = entries.iter();
    matches!(
        (pairs.next(), pairs.next()),
        (Some((key, Value::Text(_))), None) if key == BYTES_KEY
    )
}

impl serde::Serialize for Value {
    /// Two shapes, chosen by the format rather than by the value.
    ///
    /// A human-readable format gets base64 under a reserved key, because JSON
    /// has no way to say "these are bytes" and an array of numbers cannot be
    /// told apart from a list of integers. A binary format gets a real byte
    /// string, which is the property CBOR was chosen for.
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap as _;
        match self {
            Value::Text(text) => serializer.serialize_str(text),
            Value::Integer(number) => serializer.serialize_i64(*number),
            Value::Boolean(flag) => serializer.serialize_bool(*flag),
            Value::List(items) => serializer.collect_seq(items),
            Value::Map(entries) => {
                if serializer.is_human_readable() && is_binary_shaped(entries) {
                    return Err(serde::ser::Error::custom(format!(
                        "a map whose only key is {BYTES_KEY:?} cannot be written down: \
                         that shape is how binary content is recorded, and reading it \
                         back would give bytes rather than the map"
                    )));
                }
                serializer.collect_map(entries)
            }
            Value::Bytes(bytes) => {
                if serializer.is_human_readable() {
                    let mut map = serializer.serialize_map(Some(1))?;
                    map.serialize_entry(
                        BYTES_KEY,
                        &base64::engine::general_purpose::STANDARD.encode(bytes),
                    )?;
                    map.end()
                } else {
                    serializer.serialize_bytes(bytes)
                }
            }
        }
    }
}

impl<'de> serde::Deserialize<'de> for Value {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let readable = deserializer.is_human_readable();
        deserializer.deserialize_any(Reader { readable })
    }
}

/// Reads back what [`Value`]'s own `Serialize` wrote.
struct Reader {
    readable: bool,
}

impl<'de> serde::de::Visitor<'de> for Reader {
    type Value = Value;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("text, an integer, a boolean, a list, a map or bytes")
    }

    fn visit_str<E: serde::de::Error>(self, text: &str) -> Result<Value, E> {
        Ok(Value::Text(text.to_owned()))
    }

    fn visit_string<E: serde::de::Error>(self, text: String) -> Result<Value, E> {
        Ok(Value::Text(text))
    }

    fn visit_bool<E: serde::de::Error>(self, flag: bool) -> Result<Value, E> {
        Ok(Value::Boolean(flag))
    }

    fn visit_i64<E: serde::de::Error>(self, number: i64) -> Result<Value, E> {
        Ok(Value::Integer(number))
    }

    fn visit_u64<E: serde::de::Error>(self, number: u64) -> Result<Value, E> {
        i64::try_from(number)
            .map(Value::Integer)
            .map_err(|_| E::custom(format!("{number} does not fit in a signed integer")))
    }

    fn visit_bytes<E: serde::de::Error>(self, bytes: &[u8]) -> Result<Value, E> {
        Ok(Value::bytes(bytes.to_vec()))
    }

    fn visit_byte_buf<E: serde::de::Error>(self, bytes: Vec<u8>) -> Result<Value, E> {
        Ok(Value::bytes(bytes))
    }

    fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut access: A) -> Result<Value, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = access.next_element()? {
            items.push(item);
        }
        Ok(Value::List(items))
    }

    fn visit_map<A: serde::de::MapAccess<'de>>(self, mut access: A) -> Result<Value, A::Error> {
        let mut entries = BTreeMap::new();
        while let Some((key, value)) = access.next_entry::<String, Value>()? {
            entries.insert(key, value);
        }

        if self.readable
            && is_binary_shaped(&entries)
            && let Some(Value::Text(encoded)) = entries.get(BYTES_KEY)
        {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .map_err(|error| {
                    serde::de::Error::custom(format!("{BYTES_KEY} is not base64: {error}"))
                })?;
            return Ok(Value::bytes(bytes));
        }
        Ok(Value::Map(entries))
    }
}
