//! Signing the JWT a service-account key exchanges for an IAM token.
//!
//! The exchange depends on [`Signer`], not on a cryptographic library, so a
//! signer that wipes its copy of the key, or one outside the process, replaces
//! ring by implementing one trait (adr-2026-079). [`RingSigner`] is the one in
//! the tree: ring is already linked through rustls, and the `RustCrypto` `rsa`
//! crate is not admitted while RUSTSEC-2023-0071 has no patch.

use std::path::{Path, PathBuf};

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use ring::rand::SystemRandom;
use ring::signature::{RSA_PSS_SHA256, RsaKeyPair};
use serde::Serialize;

use crate::auth::AuthError;
use crate::key::{KeyError, ServiceAccountKey};

/// Signs the JWT a service-account key exchanges for an IAM token.
///
/// The algorithm is PS256 — RSASSA-PSS with SHA-256 — because it is the only one
/// the IAM endpoint accepts.
pub trait Signer: Send + Sync + std::fmt::Debug {
    /// A JWT for `audience`, valid for `ttl` seconds from `issued_at` (seconds
    /// since the epoch), signed PS256.
    ///
    /// The time is handed in so the claims are deterministic and testable
    /// rather than read from a clock the test would have to control.
    ///
    /// # Errors
    ///
    /// Returns an [`AuthError`] when the key cannot be read or is unusable, or
    /// when the signature cannot be made.
    fn jwt(&self, audience: &str, issued_at: u64, ttl: u64) -> Result<String, AuthError>;
}

/// A [`Signer`] that reads a service-account key from a file, signs with ring,
/// and keeps nothing of the key afterwards but the file's path.
///
/// The key is read again for every signature, so none sits in memory between
/// exchanges and a key file that was rotated is picked up. The buffers this
/// crate owns are wiped when the signature exists; ring's own copy of the parsed
/// key is not, which `l-yc-key-copies-unwiped` declares.
pub struct RingSigner {
    path: PathBuf,
}

impl std::fmt::Debug for RingSigner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RingSigner")
            .field("path", &self.path)
            .finish()
    }
}

impl RingSigner {
    /// A signer over the key file at `path`.
    ///
    /// The file is read once now, so that a file readable by others, a
    /// long-lived credential pasted where a key belongs, or a key ring refuses
    /// fails when the fleet is read, and not at the first apply.
    ///
    /// # Errors
    ///
    /// Returns [`AuthError::Key`] when the file cannot be used as a key.
    pub fn new(path: impl Into<PathBuf>) -> Result<Self, AuthError> {
        let path = path.into();
        let key = ServiceAccountKey::read(&path)?;
        key_pair(&key)?;
        Ok(Self { path })
    }

    /// The key file this signer reads.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Signer for RingSigner {
    fn jwt(&self, audience: &str, issued_at: u64, ttl: u64) -> Result<String, AuthError> {
        let key = ServiceAccountKey::read(&self.path)?;
        let expires_at = issued_at.checked_add(ttl).ok_or(AuthError::Clock)?;
        let header = encode(&Header {
            alg: "PS256",
            typ: "JWT",
            kid: key.id(),
        })?;
        let claims = encode(&Claims {
            iss: key.service_account_id(),
            aud: audience,
            iat: issued_at,
            exp: expires_at,
        })?;
        let signing_input = format!("{header}.{claims}");

        let pair = key_pair(&key)?;
        let mut signature = vec![0; pair.public().modulus_len()];
        pair.sign(
            &RSA_PSS_SHA256,
            &SystemRandom::new(),
            signing_input.as_bytes(),
            &mut signature,
        )
        .map_err(|_| AuthError::Sign)?;
        Ok(format!(
            "{signing_input}.{}",
            URL_SAFE_NO_PAD.encode(signature)
        ))
    }
}

/// The JWT header: the key's id and the algorithm, and nothing else.
#[derive(Serialize)]
struct Header<'a> {
    alg: &'a str,
    typ: &'a str,
    kid: &'a str,
}

/// The JWT claims: who is asking, of whom, and for how long.
#[derive(Serialize)]
struct Claims<'a> {
    iss: &'a str,
    aud: &'a str,
    iat: u64,
    exp: u64,
}

/// A JSON part of a JWT, base64url without padding.
fn encode<T: Serialize>(part: &T) -> Result<String, AuthError> {
    let json = serde_json::to_vec(part).map_err(|error| AuthError::Encode(error.to_string()))?;
    Ok(URL_SAFE_NO_PAD.encode(json))
}

/// The key as ring parses it. ring refuses an RSA key under 2048 bits, which is
/// how a weak key is refused here. The DER it parses is wiped when this returns.
fn key_pair(key: &ServiceAccountKey) -> Result<RsaKeyPair, KeyError> {
    let der = key.der()?;
    RsaKeyPair::from_pkcs8(&der).map_err(|error| {
        KeyError::BadKey(format!(
            "ring refuses it as a PKCS#8 RSA key of at least 2048 bits ({error})"
        ))
    })
}
