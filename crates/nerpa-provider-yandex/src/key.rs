//! The service-account key: the long-lived input of the credential model.
//!
//! A key enters the flow here and leaves it as a signature (adr-2026-079). It
//! is read from a file only its owner can read, held for the one signature it
//! exists for, and every buffer nerpa owns that held it is zeroized when the
//! signature exists. Copies other code makes are not wiped — ring's parsed key
//! and the JSON parser's scratch buffer — and `l-yc-key-copies-unwiped` says so.

use std::collections::BTreeSet;
use std::fs::File;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde::Deserialize;
use serde::de::IgnoredAny;
use zeroize::Zeroizing;

/// The most a key file may hold. A service-account key is a few kilobytes, so a
/// file past this is not one and is refused before it is read.
const MAX_KEY_FILE: u64 = 64 * 1024;

/// The names a long-lived credential goes by in a JSON document: an API key, a
/// static access key, a static or OAuth token. A document that carries one is
/// refused by name rather than read as if it were a key (adr-2026-079).
///
/// Each is the name as [`normalized`] and the name as the refusal says it: a
/// document that spells it `API_KEY`, `apiKey` or `api-key` is the same refusal.
const LONG_LIVED: &[(&str, &str)] = &[
    ("apikey", "api_key"),
    ("secret", "secret"),
    ("secretkey", "secret_key"),
    ("accesskey", "access_key"),
    ("statictoken", "static_token"),
    ("oauthtoken", "oauth_token"),
    ("token", "token"),
];

/// Why a key could not be read.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum KeyError {
    /// The file could not be opened or read.
    #[error("{}: {problem}", path.display())]
    Unreadable {
        /// The key file.
        path: PathBuf,
        /// What the operating system said.
        problem: String,
    },
    /// The file can be read by somebody other than its owner.
    #[error(
        "{} is readable by group or others (mode {mode:03o}); a key file has to be readable by its owner alone — chmod 600 it",
        path.display()
    )]
    TooOpen {
        /// The key file.
        path: PathBuf,
        /// Its permission bits.
        mode: u32,
    },
    /// The file is larger than a key can be.
    #[error("{} is {size} bytes, and a key file past {MAX_KEY_FILE} is not read", path.display())]
    TooLarge {
        /// The key file.
        path: PathBuf,
        /// Its size in bytes.
        size: u64,
    },
    /// The file is not a regular file.
    #[error("{} is not a regular file", path.display())]
    NotAFile {
        /// The key file.
        path: PathBuf,
    },
    /// The document holds a long-lived credential, refused by name.
    #[error(
        "a {name} is a long-lived credential and is refused; the only shape accepted is a service-account key, exchanged for a short-lived token (adr-2026-079)"
    )]
    LongLived {
        /// The name of the refused shape.
        name: &'static str,
    },
    /// The text is not a JSON object.
    #[error("the key is not a JSON object ({0})")]
    NotJson(String),
    /// The JSON holds no service-account key.
    #[error("the document is not a service-account key ({0})")]
    NotAKey(String),
    /// The private key is not a PKCS#8 PEM that can be decoded.
    #[error("the private key is not usable: {0}")]
    BadKey(String),
}

/// A service-account key, as `yc iam key create` writes it down.
///
/// The `Debug` output names the key and the account it signs for and nothing of
/// the private key (adr-2026-079). The private key is a `Zeroizing` string, so
/// the copy this struct owns is wiped when it is dropped.
pub struct ServiceAccountKey {
    id: String,
    service_account_id: String,
    private_key: Zeroizing<String>,
}

impl std::fmt::Debug for ServiceAccountKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServiceAccountKey")
            .field("id", &self.id)
            .field("service_account_id", &self.service_account_id)
            .field("private_key", &"<redacted>")
            .finish()
    }
}

/// The three fields a signature needs. The document carries more — when the key
/// was made, its algorithm, its public half — and none of that is read.
#[derive(Deserialize)]
struct Raw {
    id: String,
    service_account_id: String,
    private_key: PrivateKey,
}

/// The private key as the parser hands it over, in a string that is wiped when
/// it is dropped. It has to be a type of its own: the derive keeps each field in
/// a local until the whole object is read, and a local that holds a plain
/// `String` is dropped unwiped when a later field is missing or wrong.
struct PrivateKey(Zeroizing<String>);

impl<'de> Deserialize<'de> for PrivateKey {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(|text| Self(Zeroizing::new(text)))
    }
}

/// A JSON name with its case and its `_` and `-` taken out, so that `API_KEY`,
/// `apiKey` and `api-key` are one name.
fn normalized(name: &str) -> String {
    name.chars()
        .filter(|&c| c != '_' && c != '-')
        .flat_map(char::to_lowercase)
        .collect()
}

/// The names a JSON object carries, as [`normalized`], and nothing of what they
/// hold, so a document can be asked what it is before any of it is read as a key.
struct Names(BTreeSet<String>);

impl<'de> Deserialize<'de> for Names {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;

        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = Names;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a JSON object")
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                let mut names = BTreeSet::new();
                while let Some((name, IgnoredAny)) = map.next_entry::<String, IgnoredAny>()? {
                    names.insert(normalized(&name));
                }
                Ok(Names(names))
            }
        }

        deserializer.deserialize_map(Visitor)
    }
}

impl ServiceAccountKey {
    /// The key's id, carried as the JWT's `kid`.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The service account the key signs for, carried as the JWT's `iss`.
    #[must_use]
    pub fn service_account_id(&self) -> &str {
        &self.service_account_id
    }

    /// Reads a key from the JSON it was written down in.
    ///
    /// A long-lived shape — an API key, a static access key, a static or OAuth
    /// token — is refused by name before anything is read as a key. The private
    /// key's PEM is not decoded here; the signature is the moment it has to be
    /// usable.
    ///
    /// # Errors
    ///
    /// Returns [`KeyError::NotJson`] when the text is not a JSON object,
    /// [`KeyError::LongLived`] when it names a long-lived credential, and
    /// [`KeyError::NotAKey`] when it lacks the fields of a service-account
    /// key. No message repeats the text: a document pasted wrongly must not
    /// land in a log.
    pub fn from_json(text: &str) -> Result<Self, KeyError> {
        let names: Names =
            serde_json::from_str(text).map_err(|error| KeyError::NotJson(place(&error)))?;
        for &(normal, name) in LONG_LIVED {
            if names.0.contains(normal) {
                return Err(KeyError::LongLived { name });
            }
        }
        let raw: Raw =
            serde_json::from_str(text).map_err(|error| KeyError::NotAKey(place(&error)))?;
        Ok(Self {
            id: raw.id,
            service_account_id: raw.service_account_id,
            private_key: raw.private_key.0,
        })
    }

    /// Reads a key from the file at `path`.
    ///
    /// The file has to be a regular file no larger than a key can be, and
    /// readable by its owner alone; the permission is read from the opened file
    /// itself, so it cannot change between the check and the read. The bytes are
    /// held in a buffer that is wiped when this returns.
    ///
    /// # Errors
    ///
    /// Returns [`KeyError::Unreadable`], [`KeyError::NotAFile`],
    /// [`KeyError::TooLarge`] or [`KeyError::TooOpen`] for the file, and what
    /// [`ServiceAccountKey::from_json`] returns for its contents.
    pub fn read(path: &Path) -> Result<Self, KeyError> {
        let unreadable = |error: std::io::Error| KeyError::Unreadable {
            path: path.to_path_buf(),
            problem: error.to_string(),
        };
        let not_a_file = || KeyError::NotAFile {
            path: path.to_path_buf(),
        };
        // Looked at before it is opened, so that a FIFO or a device named by
        // mistake is refused and not waited on. The opened file is looked at
        // again below: this one is a courtesy, that one is the check.
        if !std::fs::metadata(path).map_err(unreadable)?.is_file() {
            return Err(not_a_file());
        }
        let file = File::open(path).map_err(unreadable)?;
        let metadata = file.metadata().map_err(unreadable)?;
        if !metadata.is_file() {
            return Err(not_a_file());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = metadata.permissions().mode() & 0o777;
            if mode & 0o077 != 0 {
                return Err(KeyError::TooOpen {
                    path: path.to_path_buf(),
                    mode,
                });
            }
        }
        if metadata.len() > MAX_KEY_FILE {
            return Err(KeyError::TooLarge {
                path: path.to_path_buf(),
                size: metadata.len(),
            });
        }
        // The buffer is as large as the largest key file, whatever this one is,
        // and the read is capped one byte past it: a file that grows between the
        // size check and the read cannot make the buffer grow by copying, and a
        // grown buffer leaves its old self unwiped.
        let too_large = |size: u64| KeyError::TooLarge {
            path: path.to_path_buf(),
            size,
        };
        let most = usize::try_from(MAX_KEY_FILE).map_err(|_| too_large(MAX_KEY_FILE))?;
        let mut bytes = Zeroizing::new(Vec::with_capacity(most.saturating_add(1)));
        file.take(MAX_KEY_FILE.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(unreadable)?;
        if bytes.len() > most {
            return Err(too_large(MAX_KEY_FILE.saturating_add(1)));
        }
        let text = std::str::from_utf8(&bytes)
            .map_err(|_| KeyError::NotJson("the file is not text".to_owned()))?;
        Self::from_json(text)
    }

    /// The private key as DER, decoded from its PEM into a buffer that is wiped
    /// when it is dropped.
    ///
    /// A key file written by `yc iam key create` carries a marker line before the
    /// armor — `PLEASE DO NOT REMOVE THIS LINE!` — which is not part of the key.
    pub(crate) fn der(&self) -> Result<Zeroizing<Vec<u8>>, KeyError> {
        let mut body = Zeroizing::new(String::with_capacity(self.private_key.len()));
        for line in self.private_key.lines() {
            let line = line.trim();
            if line.starts_with("-----") || line.starts_with("PLEASE DO NOT REMOVE THIS LINE!") {
                continue;
            }
            body.push_str(line);
        }
        let mut der = Zeroizing::new(Vec::with_capacity(body.len() / 4 * 3 + 3));
        STANDARD
            .decode_vec(body.as_bytes(), &mut der)
            .map_err(|_| KeyError::BadKey("the PEM is not base64".to_owned()))?;
        Ok(der)
    }
}

/// Where in a text a JSON error is, and of what kind — and nothing of what the
/// text says, because serde's own message can quote a string from it, and the
/// string may be a key or a token.
pub(crate) fn place(error: &serde_json::Error) -> String {
    format!(
        "{:?} at line {} column {}",
        error.classify(),
        error.line(),
        error.column()
    )
}
