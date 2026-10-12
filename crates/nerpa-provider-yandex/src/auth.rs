//! The IAM token exchange: a service account key becomes a short-lived token.
//!
//! A long-lived key is never a mode here. The provider holds the key only long
//! enough to sign one JWT, exchanges it for an IAM token, and forgets the key;
//! what it carries afterwards is the token, which expires. The signing is done
//! here, by hand, so every byte that is signed is visible and testable.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rsa::RsaPrivateKey;
use rsa::pkcs1v15::SigningKey;
use rsa::pkcs8::DecodePrivateKey;
use rsa::signature::{SignatureEncoding, Signer};
use serde::Serialize;
use sha2::Sha256;

/// The IAM endpoint that exchanges a signed JWT for a token.
const IAM_TOKEN_URL: &str = "https://iam.api.cloud.yandex.net/iam/v1/tokens";

/// A service account key, as Yandex Cloud hands it out.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ServiceAccountKey {
    /// The key id, carried as the JWT's `kid`.
    pub id: String,
    /// The service account the key belongs to, carried as the JWT's `iss`.
    #[serde(rename = "service_account_id")]
    pub service_account_id: String,
    /// The PEM private key. Held only to sign, then forgotten.
    #[serde(rename = "private_key")]
    pub private_key: String,
}

/// A short-lived IAM token.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct IamToken {
    /// The token, carried in the `Authorization: Bearer` header afterwards.
    #[serde(rename = "iamToken")]
    pub token: String,
    /// When it stops working.
    #[serde(rename = "expiresAt")]
    pub expires_at: String,
}

/// Why a token could not be obtained.
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    /// The key could not be read or signed.
    #[error("the service account key is not usable: {0}")]
    Key(String),
    /// The exchange itself failed.
    #[error("the IAM token exchange failed: {0}")]
    Exchange(String),
}

/// The JWT header, which is the key id and the algorithm and nothing else.
#[derive(Debug, Clone, Serialize)]
struct JwtHeader<'a> {
    alg: &'a str,
    typ: &'a str,
    kid: &'a str,
}

/// The JWT claims: who is asking, to whom, and for how long.
#[derive(Debug, Clone, Serialize)]
struct JwtClaims<'a> {
    iss: &'a str,
    aud: &'a str,
    iat: u64,
    exp: u64,
}

/// Signs the JWT a service account key exchanges for an IAM token.
///
/// `now` and `ttl` are seconds since the epoch, handed in so the signature is
/// deterministic and testable rather than read from a clock the test would have
/// to control.
pub fn signed_jwt(key: &ServiceAccountKey, now: u64, ttl: u64) -> Result<String, AuthError> {
    let header = URL_SAFE_NO_PAD.encode(
        serde_json::to_vec(&JwtHeader {
            alg: "RS256",
            typ: "JWT",
            kid: &key.id,
        })
        .map_err(|e| AuthError::Key(e.to_string()))?,
    );
    let claims = URL_SAFE_NO_PAD.encode(
        serde_json::to_vec(&JwtClaims {
            iss: &key.service_account_id,
            aud: IAM_TOKEN_URL,
            iat: now,
            exp: now + ttl,
        })
        .map_err(|e| AuthError::Key(e.to_string()))?,
    );
    let signing_input = format!("{header}.{claims}");

    let private_key = RsaPrivateKey::from_pkcs8_pem(&key.private_key)
        .map_err(|e| AuthError::Key(e.to_string()))?;
    let signing_key = SigningKey::<Sha256>::new(private_key);
    let signature = signing_key.sign(signing_input.as_bytes());

    Ok(format!(
        "{signing_input}.{}",
        URL_SAFE_NO_PAD.encode(signature.to_bytes())
    ))
}

/// Exchanges a signed JWT for a short-lived IAM token.
pub fn token_for(jwt: &str) -> Result<IamToken, AuthError> {
    let response = ureq::post(IAM_TOKEN_URL)
        .send_json(serde_json::json!({ "jwt": jwt }))
        .map_err(|e| AuthError::Exchange(e.to_string()))?;
    response
        .into_json::<IamToken>()
        .map_err(|e| AuthError::Exchange(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsa::RsaPublicKey;
    use rsa::pkcs1v15::{Signature, VerifyingKey};
    use rsa::pkcs8::EncodePrivateKey;
    use rsa::rand_core::OsRng;
    use rsa::signature::DigestVerifier;
    use sha2::Digest;

    #[test]
    fn a_signed_jwt_verifies_with_the_same_key() {
        let private = RsaPrivateKey::new(&mut OsRng, 2048).expect("a key");
        let public = RsaPublicKey::from(&private);
        let pem = private
            .to_pkcs8_pem(rsa::pkcs8::LineEnding::LF)
            .expect("a pem");

        let key = ServiceAccountKey {
            id: "ajekey".to_owned(),
            service_account_id: "ajeservice".to_owned(),
            private_key: pem.to_string(),
        };

        let jwt = signed_jwt(&key, 1_700_000_000, 3_600).expect("signed");

        let parts: Vec<&str> = jwt.split('.').collect();
        assert_eq!(parts.len(), 3, "header.claims.signature");
        let signing_input = format!("{}.{}", parts[0], parts[1]);
        let signature = URL_SAFE_NO_PAD.decode(parts[2]).expect("base64");
        let signature = Signature::try_from(signature.as_slice()).expect("a signature");

        let verifying = VerifyingKey::<Sha256>::new(public);
        let mut hasher = Sha256::new();
        hasher.update(signing_input.as_bytes());
        verifying
            .verify_digest(hasher, &signature)
            .expect("the signature holds");
    }
}
