//! The IAM token exchange: a service account key becomes a short-lived token.
//!
//! A long-lived key enters the flow here and leaves it as a short-lived token
//! (070). The key is held only long enough to sign one JWT — the DER it parses
//! to is zeroized the moment the signature exists — and what is carried
//! afterwards is the token, which expires. The signing is done here, by hand,
//! so every byte that is signed is visible and testable.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use ring::rand::SystemRandom;
use ring::signature::{RSA_PSS_SHA256, RsaKeyPair};
use serde::Serialize;
use tonic::transport::Endpoint;

use crate::yandex::cloud::iam::v1::CreateIamTokenRequest;
use crate::yandex::cloud::iam::v1::create_iam_token_request::Identity;
use crate::yandex::cloud::iam::v1::iam_token_service_client::IamTokenServiceClient;

/// The IAM service the JWT is exchanged at, over gRPC.
const IAM_ENDPOINT: &str = "https://iam.api.cloud.yandex.net";

/// The `aud` claim: the URL the IAM documentation says the token is requested
/// at. Carried verbatim, as the endpoint expects it.
const IAM_AUDIENCE: &str = "https://iam.api.cloud.yandex.net/iam/v1/tokens";

/// The most a call against the IAM service may take: no answer is an error,
/// not a hang (`INV-REACH-001`).
const IAM_TIMEOUT: Duration = Duration::from_secs(60);

/// A service account key, as Yandex Cloud hands it out.
///
/// The `Debug` output redacts the private key: a key is a credential, and
/// debug output is the last place one should appear (070).
#[derive(Clone, serde::Deserialize)]
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

impl std::fmt::Debug for ServiceAccountKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServiceAccountKey")
            .field("id", &self.id)
            .field("service_account_id", &self.service_account_id)
            .field("private_key", &"<redacted>")
            .finish_non_exhaustive()
    }
}

/// A short-lived IAM token, as the exchange hands it back.
///
/// The `Debug` output redacts the token (070).
#[derive(Clone)]
pub struct IamToken {
    /// The token, carried in the `Authorization: Bearer` header afterwards.
    token: String,
    /// When it stops working.
    expires_at: SystemTime,
}

impl IamToken {
    /// The token, for the `Authorization: Bearer` header.
    #[must_use]
    pub fn token(&self) -> &str {
        &self.token
    }

    /// When it stops working.
    #[must_use]
    pub fn expires_at(&self) -> SystemTime {
        self.expires_at
    }
}

impl std::fmt::Debug for IamToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IamToken")
            .field("token", &"<redacted>")
            .field("expires_at", &self.expires_at)
            .finish_non_exhaustive()
    }
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
/// The algorithm is PS256 — RSASSA-PSS with SHA-256 and a 32-byte salt —
/// because it is the only one the endpoint accepts. `now` and `ttl` are
/// seconds since the epoch, handed in so the signature is deterministic and
/// testable rather than read from a clock the test would have to control.
///
/// # Errors
///
/// Returns [`AuthError::Key`] when the key cannot be read or signed.
pub fn signed_jwt(key: &ServiceAccountKey, now: u64, ttl: u64) -> Result<String, AuthError> {
    let header = URL_SAFE_NO_PAD.encode(
        serde_json::to_vec(&JwtHeader {
            alg: "PS256",
            typ: "JWT",
            kid: &key.id,
        })
        .map_err(|e| AuthError::Key(e.to_string()))?,
    );
    let claims = URL_SAFE_NO_PAD.encode(
        serde_json::to_vec(&JwtClaims {
            iss: &key.service_account_id,
            aud: IAM_AUDIENCE,
            iat: now,
            exp: now + ttl,
        })
        .map_err(|e| AuthError::Key(e.to_string()))?,
    );
    let signing_input = format!("{header}.{claims}");

    // The PEM becomes DER once, and the DER is zeroized the moment the
    // signature exists: the key's bytes do not outlive the one signature
    // they exist for (070).
    let mut der = pem_to_der(&key.private_key).map_err(AuthError::Key)?;
    let key_pair = RsaKeyPair::from_pkcs8(&der).map_err(|e| AuthError::Key(e.to_string()))?;
    let mut signature = vec![0; key_pair.public().modulus_len()];
    key_pair
        .sign(
            &RSA_PSS_SHA256,
            &SystemRandom::new(),
            signing_input.as_bytes(),
            &mut signature,
        )
        .map_err(|e| AuthError::Key(e.to_string()))?;
    der.fill(0);

    Ok(format!(
        "{signing_input}.{}",
        URL_SAFE_NO_PAD.encode(signature)
    ))
}

/// The DER the PEM spells: everything between the armor lines, decoded.
fn pem_to_der(pem: &str) -> Result<Vec<u8>, String> {
    let body: String = pem
        .lines()
        .filter(|line| !line.starts_with("-----"))
        .collect();
    STANDARD
        .decode(body.trim())
        .map_err(|e| format!("the PEM is not readable: {e}"))
}

/// The wall clock now, in seconds since the epoch.
///
/// # Errors
///
/// Returns [`AuthError::Key`] when the clock reads before the epoch, which
/// no signature should be taken under.
pub fn now_seconds() -> Result<u64, AuthError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .map_err(|e| AuthError::Key(format!("the clock reads before the epoch: {e}")))
}

/// Exchanges a signed JWT for a short-lived IAM token.
///
/// The exchange runs over the IAM gRPC service — the same transport the
/// client reads resources through — speaking TLS as the `https` endpoint
/// requires, and bounded by a one-minute timeout.
///
/// # Errors
///
/// Returns [`AuthError::Exchange`] when the endpoint refuses the JWT, is
/// unreachable, or the answer is not a token.
pub async fn token_for(jwt: &str) -> Result<IamToken, AuthError> {
    let channel = Endpoint::from_shared(IAM_ENDPOINT.to_owned())
        .map_err(|e| AuthError::Exchange(e.to_string()))?
        .connect_timeout(IAM_TIMEOUT)
        .timeout(IAM_TIMEOUT)
        .connect()
        .await
        .map_err(|e| AuthError::Exchange(e.to_string()))?;
    let mut client = IamTokenServiceClient::new(channel);
    let response = client
        .create(CreateIamTokenRequest {
            identity: Some(Identity::Jwt(jwt.to_owned())),
        })
        .await
        .map_err(|e| AuthError::Exchange(e.to_string()))?
        .into_inner();
    let expires_at = SystemTime::try_from(response.expires_at.ok_or_else(|| {
        AuthError::Exchange("the token service answered without an expiry".to_owned())
    })?)
    .map_err(|e| AuthError::Exchange(e.to_string()))?;
    Ok(IamToken {
        token: response.iam_token,
        expires_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ring::signature::{RSA_PSS_2048_8192_SHA256, UnparsedPublicKey};

    /// A fixed PKCS#8 key made for tests only: the private half signs, the
    /// public half verifies, and neither is a credential anybody uses.
    const KEY_PEM: &str = "\
-----BEGIN PRIVATE KEY-----
MIIEvAIBADANBgkqhkiG9w0BAQEFAASCBKYwggSiAgEAAoIBAQCNOxt1XCNROG3J
oNRCc0ehoX9URZU/rG/XVuEdV/pGQjmq1lcoG1b9nSky8mL9upT5SXcFTwZk97KW
yH4Tj54BLNmNhPwX38ggJuz/SjVMXTBAIeCoGIJorfZlrL132vBZvAusmllIZSft
5bsCBDOYKzh6djarqpAkc86jH/DtR0+wkTwsz2FiSu39o7v6/OWgmstyYvuq2CYK
f0yUKrF8TI7TbpDCjxGCHzl/D6SmuOwACwkebt1RbHkHaPgqua4oqpI7VBsp8/QR
h+xsb9PqsQNjxY1QShRO7B0XdvzrjLQkxZU7C3/sbvdA0zWKXWSvnnfUDEMV/O8U
R7XECdNhAgMBAAECggEADL+1LtbuaTqKzpSyFH2Ljc+iSt0tJ95NCkdRcvCqKwV3
ZzCKFZjCNYqmWW7/+J+LKyWylMwZa6vgdzQKDYH9ZWXDDb9fclhFHIHnCvSL8yTG
YkOsn//2ZbSudWcug09EOxV6iE42mQ5omFUQmJIAhlAAfS2GqeBvupqpoJgVi5Wu
C38awBc3g8T6NAbXRp6c6OcyW6+sHyz7jomwwIkvWwZLedm6UP+9xYCCYPOb44Bf
PYwiSx2wXpu1a7BlhJuyPvtNGUUqGHwBP4fsM50pp9kr9CQOxg7JtxjGwWE7Sahe
kVCW/5tj4WwkIQ+tP1e8OOYCBm2K11wrP5zu4/MPQQKBgQC/OUKhnYgiXPcGUAaA
GnIF/xIJbNulZHBwNon+5KKzJLlKhhq6TlZXIjduBYc16KGcbfD557RK7C1Nf3mP
7jOfc7+xwM4f2acaQFIJpI36/DjTX1MJFw/3amENWT7r47jekcC1EJGj0SeWC2Mk
IKzWPs9aLUn020xfpHuuiqrP5QKBgQC9EoTbVIQEj26mCJ1Dmlv8jtX866aAYr57
sMRZiFQ+jKR3tBc/v206O/olw7PXgB0TxYlc9q+wgBKC96wyMFrVpO0fobeDSn9R
cUScLh6MCS0TOkceLpnXZvH4SFhny/QE7ZTfdw0udlK8jFZZzo7+6/Pbgx8G4I17
tsGI1IxlzQKBgGGWsWCVarJOy2qXUJnz5RkWUXgp6/Z+SbuaudYxjK7sIPvdcKp/
7CRPvAYUICrs6/1PRlnjW+mYmo4gbkN8BzBq+/uVMBylEVieVr/BEQxzx/Rng5Ly
CB32Vk1frHLqJqC/tZ8yM4Fs7kzuxsvbORmluGCHGGUzvchZwx0Se5XZAoGAJGTX
K/zVuKb/TnhkBSpGQJQh/qBBmX7oWZjKAs3qIWwjEGObAgtP9CCtYu2LxZSEhFTG
vx1A7kGIzqrr+3ExWuR3PFp9n4kMftnFsiZyfNQbWBMks1uXA05ujzMlyT0KCJTO
zkD1SZc/D8oXqQvInKtAcmWH6GP69/BSyorXR5UCgYBUdRa1fb2nW2WYua8olY4+
36dAz/qOQnhw8XX40lCjupMpEMw5KHhHzzT9lrxUT3Uw1q4dH9VIy0gBg4WdtCGa
lvQlisiuEKZy3XkFO412zbEW9pJPRky+zi+sy+9VvbvRdUp62X9NgiZ7CD60iltA
NFOvnrJCq7mtBmQIvPZixQ==
-----END PRIVATE KEY-----
";

    /// The public half of [`KEY_PEM`], as ASN.1 `RSAPublicKey` — the format
    /// ring's verification reads — lives beside this file in
    /// `rsa_public_key_test.der`, made with
    /// `openssl rsa -RSAPublicKey_out -outform der`.
    fn a_key() -> ServiceAccountKey {
        ServiceAccountKey {
            id: "ajekey".to_owned(),
            service_account_id: "ajeservice".to_owned(),
            private_key: KEY_PEM.to_owned(),
        }
    }

    #[test]
    fn a_signed_jwt_verifies_with_the_public_key() {
        let jwt = signed_jwt(&a_key(), 1_700_000_000, 3_600).expect("signed");

        let mut parts = jwt.split('.');
        let header = parts.next().expect("a header");
        let claims = parts.next().expect("claims");
        let signature = parts.next().expect("a signature");
        assert!(parts.next().is_none(), "exactly three parts");
        let signing_input = format!("{header}.{claims}");
        let signature = URL_SAFE_NO_PAD.decode(signature).expect("base64");
        let public = include_bytes!("rsa_public_key_test.der");

        UnparsedPublicKey::new(&RSA_PSS_2048_8192_SHA256, public)
            .verify(signing_input.as_bytes(), &signature)
            .expect("the signature holds");
    }

    #[test]
    fn no_debug_prints_key_or_token_material() {
        let key = a_key();
        let key_debug = format!("{key:?}");
        assert!(
            !key_debug.contains("PRIVATE KEY"),
            "the key is redacted: {key_debug}"
        );
        assert!(
            !key_debug.contains("MIIE"),
            "the key is redacted: {key_debug}"
        );

        let token = IamToken {
            token: "t1.token-text".to_owned(),
            expires_at: SystemTime::now() + Duration::from_secs(3_600),
        };
        let token_debug = format!("{token:?}");
        assert!(
            !token_debug.contains("t1.token-text"),
            "the token is redacted: {token_debug}"
        );
    }
}
