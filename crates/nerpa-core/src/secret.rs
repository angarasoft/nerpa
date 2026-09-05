//! Values that must never be written down.
//!
//! Terraform's state file is a plaintext dump of every credential the
//! configuration touched, and the answer has always been to encrypt the whole
//! file and hope. The complaint was never that state is unencrypted. It is that
//! a value nobody needed to keep was kept anyway.
//!
//! So a secret enters the model as a [`Source`] — where to fetch it — and leaves
//! as a [`Digest`], which answers the only question state has to answer about it:
//! whether it changed. The value itself lives in memory for as long as a provider
//! needs it and is never handed to anything that persists.
//!
//! # Why the digest is keyed
//!
//! An unkeyed digest of a short secret is a lookup, not a one-way function: a
//! dictionary of likely passwords defeats it in seconds. The key makes a digest
//! that escapes into a log, a diff or a pasted plan useless to whoever finds it.
//!
//! It offers nothing against somebody holding the state file, because the key is
//! in the state file. That is not an oversight and no stronger claim is made: a
//! state file is not a place to put a secret, which is the point of this module.

use std::fmt;

use crate::value::Value;

/// Where a secret value comes from.
///
/// Non-exhaustive: a secret computed from other secrets — a rendered template,
/// a formatted connection string — is a second source, and it arrives with the
/// templating work rather than being guessed at now. See decision 0027.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Source {
    /// Held by a secret store, and fetched from it when it is needed.
    Stored {
        /// Which store — `lockbox`, `vault`, `file`.
        provider: String,
        /// Where in that store.
        path: String,
        /// Which version, where the store has them. `None` means whatever is
        /// current, which is convenient and means a plan can go stale.
        version: Option<String>,
    },
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Source::Stored {
                provider,
                path,
                version,
            } => match version {
                Some(version) => write!(f, "{provider}:{path}@{version}"),
                None => write!(f, "{provider}:{path}"),
            },
        }
    }
}

/// The number of bytes in a key and in a digest.
pub(crate) const LENGTH: usize = 32;

/// The key a state file's digests are taken under.
///
/// One per state file, made when the file is first written and kept for its
/// life: digests taken under two different keys never compare equal, so changing
/// it would report every secret as changed.
#[derive(Clone, PartialEq, Eq)]
pub struct Key([u8; LENGTH]);

/// The operating system would not supply random bytes.
#[derive(Debug, thiserror::Error)]
#[error("no randomness available for a digest key: {0}")]
pub struct NoRandomness(String);

impl Key {
    /// A new key, from the operating system's randomness.
    ///
    /// # Errors
    ///
    /// Returns [`NoRandomness`] if the operating system would not supply any.
    /// There is no fallback on purpose: a predictable key is worse than none,
    /// because it looks like protection.
    pub fn fresh() -> Result<Self, NoRandomness> {
        let mut bytes = [0u8; LENGTH];
        getrandom::fill(&mut bytes).map_err(|error| NoRandomness(error.to_string()))?;
        Ok(Self(bytes))
    }

    /// A key read back from a state file.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; LENGTH]) -> Self {
        Self(bytes)
    }

    /// Written as hexadecimal, which is how a state file carries it.
    #[must_use]
    pub fn to_hex(&self) -> String {
        to_hex(&self.0)
    }

    /// Read back from hexadecimal.
    #[must_use]
    pub fn from_hex(text: &str) -> Option<Self> {
        from_hex(text).map(Self)
    }
}

/// Deliberately opaque: a key that prints itself lands in a log eventually.
impl fmt::Debug for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Key(…)")
    }
}

/// A keyed digest of a value that is not itself recorded.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Digest([u8; LENGTH]);

impl Digest {
    /// The digest of a value, under a state file's key.
    #[must_use]
    pub fn of(key: &Key, value: &Value) -> Self {
        let mut encoded = Vec::new();
        crate::value::encode(value, &mut encoded);
        Self(*blake3::keyed_hash(&key.0, &encoded).as_bytes())
    }

    /// Written as hexadecimal.
    #[must_use]
    pub fn to_hex(&self) -> String {
        to_hex(&self.0)
    }

    /// Read back from hexadecimal.
    #[must_use]
    pub fn from_hex(text: &str) -> Option<Self> {
        from_hex(text).map(Self)
    }

    /// The bytes, for a caller that is hashing this digest into a larger one.
    pub(crate) fn raw(&self) -> &[u8; LENGTH] {
        &self.0
    }
}

impl fmt::Debug for Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Digest({})", self.to_hex())
    }
}

impl fmt::Display for Digest {
    /// The first eight characters, which is enough to tell two apart by eye and
    /// not enough to be worth attacking.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.to_hex().get(..8).unwrap_or_default())
    }
}

impl serde::Serialize for Digest {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> serde::Deserialize<'de> for Digest {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = <String as serde::Deserialize>::deserialize(deserializer)?;
        Digest::from_hex(&text)
            .ok_or_else(|| serde::de::Error::custom(format!("{text:?} is not a digest")))
    }
}

/// A value as it may be written down — in state, or in a plan.
///
/// This is the type that makes "state never holds a secret value" true by
/// construction rather than by everybody remembering. There is no way back from
/// [`Recorded::Secret`] to the value it was taken from.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Recorded {
    /// The value itself, because it is not a secret.
    Plain(Value),
    /// A secret, reduced to a digest that says only whether it changed.
    Secret(Digest),
}

impl Recorded {
    /// The value, if there is one that may be shown.
    #[must_use]
    pub fn plain(&self) -> Option<&Value> {
        match self {
            Recorded::Plain(value) => Some(value),
            Recorded::Secret(_) => None,
        }
    }

    /// Whether this is a secret.
    #[must_use]
    pub fn is_secret(&self) -> bool {
        matches!(self, Recorded::Secret(_))
    }
}

impl Recorded {
    /// A whole set of attributes, none of which is a secret.
    ///
    /// The ordinary case: a resource acquires a secret attribute only where a
    /// declaration says so, and most never do.
    #[must_use]
    pub fn all(
        attributes: impl IntoIterator<Item = (String, Value)>,
    ) -> std::collections::BTreeMap<String, Self> {
        attributes
            .into_iter()
            .map(|(name, value)| (name, Recorded::Plain(value)))
            .collect()
    }
}

impl fmt::Display for Recorded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Recorded::Plain(value) => write!(f, "{value}"),
            Recorded::Secret(_) => f.write_str("(secret)"),
        }
    }
}

/// A secret as a plan carries it: where it comes from, and what it hashes to.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Sealed {
    source: Source,
    digest: Digest,
}

impl Sealed {
    /// Seals a fetched value: the digest is kept, the value is not.
    #[must_use]
    pub fn new(source: Source, key: &Key, value: &Value) -> Self {
        Self {
            source,
            digest: Digest::of(key, value),
        }
    }

    /// Where it came from.
    #[must_use]
    pub fn source(&self) -> &Source {
        &self.source
    }

    /// What it hashes to.
    #[must_use]
    pub fn digest(&self) -> Digest {
        self.digest
    }
}

impl fmt::Display for Sealed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(secret from {})", self.source)
    }
}

pub(crate) fn to_hex(bytes: &[u8; LENGTH]) -> String {
    use fmt::Write as _;
    bytes.iter().fold(String::new(), |mut text, byte| {
        // Writing into a String cannot fail, and the result is discarded rather
        // than unwrapped so that the lint against `unwrap` stays meaningful.
        let _ = write!(text, "{byte:02x}");
        text
    })
}

pub(crate) fn from_hex(text: &str) -> Option<[u8; LENGTH]> {
    if text.len() != LENGTH * 2 {
        return None;
    }
    let mut bytes = [0u8; LENGTH];
    let (pairs, _) = text.as_bytes().as_chunks::<2>();
    for (slot, pair) in bytes.iter_mut().zip(pairs) {
        let pair = std::str::from_utf8(pair).ok()?;
        *slot = u8::from_str_radix(pair, 16).ok()?;
    }
    Some(bytes)
}

/// Why a secret could not be fetched.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FetchError {
    /// The store has nothing at that path.
    #[error("no secret at {at}")]
    Missing {
        /// Where it was looked for. Not named `source`, which `thiserror` would
        /// take for the underlying error.
        at: Source,
    },
    /// The store could not be asked.
    #[error("could not read {at}: {problem}")]
    Unavailable {
        /// Where it was looked for.
        at: Source,
        /// What went wrong.
        problem: String,
    },
}

/// Where secret values actually come from.
///
/// Planning has to fetch, because the only way to say whether a secret changed
/// is to hash what is there now and compare. The value is digested and dropped;
/// nothing downstream of the planner is given it.
pub trait SecretStore: fmt::Debug + Send + Sync {
    /// Fetches the value behind a source.
    ///
    /// # Errors
    ///
    /// Returns [`FetchError`] if there is nothing there, or if the store could
    /// not be asked at all.
    fn fetch(&self, source: &Source) -> Result<Value, FetchError>;
}

/// A store that holds what it was given, for tests and for `nerpa` without a
/// real store configured.
///
/// Required of the trait's own module rather than left to each implementation:
/// a boundary with no fake behind it is a boundary every test has to go around,
/// and tests that go around a boundary stop covering it.
#[derive(Debug, Default)]
pub struct Remembered {
    entries: std::collections::BTreeMap<String, Value>,
}

impl Remembered {
    /// A store holding nothing, and const so that it can be a `static`.
    ///
    /// What a run has until a real store is configured: a declaration that names
    /// a secret gets told there is nothing there, which is the truth.
    pub const EMPTY: Self = Self {
        entries: std::collections::BTreeMap::new(),
    };

    /// A store holding nothing.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Puts a value at a path.
    #[must_use]
    pub fn holding(mut self, path: impl Into<String>, value: impl Into<Value>) -> Self {
        self.entries.insert(path.into(), value.into());
        self
    }
}

impl SecretStore for Remembered {
    fn fetch(&self, source: &Source) -> Result<Value, FetchError> {
        let Source::Stored { path, .. } = source;
        self.entries
            .get(path)
            .cloned()
            .ok_or_else(|| FetchError::Missing { at: source.clone() })
    }
}

/// What planning needs in order to handle secrets without keeping them.
///
/// The key and the store travel together because neither is any use alone: a
/// digest under the wrong key compares unequal to every other, and a value with
/// no key to digest it under would have to be written down.
#[derive(Debug, Clone, Copy)]
pub struct Secrets<'a> {
    key: &'a Key,
    store: &'a dyn SecretStore,
}

impl<'a> Secrets<'a> {
    /// Binds a state file's key to the store its secrets come from.
    #[must_use]
    pub fn new(key: &'a Key, store: &'a dyn SecretStore) -> Self {
        Self { key, store }
    }

    /// The key digests are taken under.
    #[must_use]
    pub fn key(&self) -> &Key {
        self.key
    }

    /// Fetches and seals: the digest is kept, the value is dropped here.
    ///
    /// # Errors
    ///
    /// Returns [`FetchError`] if the store has nothing there or could not be
    /// asked.
    // ENFORCES: INV-SECRET-002
    pub fn seal(&self, source: &Source) -> Result<Sealed, FetchError> {
        let value = self.store.fetch(source)?;
        Ok(Sealed::new(source.clone(), self.key, &value))
    }

    /// The value itself, for the one caller that must have it: the step that
    /// hands a provider what it is to write.
    ///
    /// Deliberately the only way to get one, and deliberately not on the way to
    /// anything that persists — [`Sealed`] is what the plan and state get.
    ///
    /// # Errors
    ///
    /// Returns [`FetchError`] if the store has nothing there or could not be
    /// asked.
    pub fn fetch(&self, source: &Source) -> Result<Value, FetchError> {
        self.store.fetch(source)
    }

    /// The digest of an observed value, for comparing against a sealed one.
    #[must_use]
    pub fn digest(&self, value: &Value) -> Digest {
        Digest::of(self.key, value)
    }
}
