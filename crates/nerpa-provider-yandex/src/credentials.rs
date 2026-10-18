//! The credential the provider accepts, and the shapes it refuses.
//!
//! A credential enters the flow here as a service-account key and leaves it,
//! in [`crate::auth`], as a short-lived token (070). A long-lived shape — an
//! API key or a static token — is refused by name, so a person who pastes one
//! is told exactly which shape was refused rather than silently served a mode
//! the model does not carry.

use crate::auth::ServiceAccountKey;

/// The credential the provider was given.
///
/// The `Debug` output names the shape and nothing of the key (070).
#[derive(Clone)]
pub struct Credentials {
    service_account_key: ServiceAccountKey,
}

impl Credentials {
    /// The service-account key the exchange signs with.
    #[must_use]
    pub fn service_account_key(&self) -> &ServiceAccountKey {
        &self.service_account_key
    }

    /// Reads the credential from the JSON it was written down in.
    ///
    /// A long-lived shape is refused by name before anything is parsed: the
    /// only supported shape is a service-account key, exchanged for a
    /// short-lived token (070). The key's PEM is not validated here — the
    /// exchange is the moment it must be usable.
    ///
    /// # Errors
    ///
    /// Returns [`CredentialsError::LongLived`] when the JSON holds an API key
    /// or a static token, [`CredentialsError::NotAKey`] when it holds no
    /// service-account key, and [`CredentialsError::Unreadable`] when it is
    /// not JSON at all.
    pub fn from_json(text: &str) -> Result<Self, CredentialsError> {
        let raw: serde_json::Value =
            serde_json::from_str(text).map_err(|e| CredentialsError::Unreadable(e.to_string()))?;
        for name in ["api_key", "api-key", "token", "static_token"] {
            if raw.get(name).is_some() {
                return Err(CredentialsError::LongLived { name });
            }
        }
        let service_account_key = serde_json::from_value::<ServiceAccountKey>(raw)
            .map_err(|e| CredentialsError::NotAKey(e.to_string()))?;
        Ok(Self {
            service_account_key,
        })
    }
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credentials")
            .field("shape", &"service_account_key")
            .finish_non_exhaustive()
    }
}

/// Why a credential could not be read.
#[derive(Debug, thiserror::Error)]
pub enum CredentialsError {
    /// The text is not JSON.
    #[error("the credential is not readable: {0}")]
    Unreadable(String),
    /// The JSON holds a long-lived credential, refused by name (070).
    #[error(
        "a {name} is a long-lived credential and is refused; the only supported shape is a service-account key, exchanged for a short-lived token (070)"
    )]
    LongLived {
        /// The name of the refused shape.
        name: &'static str,
    },
    /// The JSON holds no service-account key.
    #[error("the credential is not a service-account key: {0}")]
    NotAKey(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_lived_credential_is_refused_by_name() {
        let api_key = Credentials::from_json(r#"{"api_key": "AQVN-abc"}"#)
            .expect_err("an API key is refused");
        assert!(
            api_key.to_string().contains("api_key"),
            "the refusal names the shape: {api_key}"
        );

        let token = Credentials::from_json(r#"{"token": "t1.abc"}"#)
            .expect_err("a static token is refused");
        assert!(
            token.to_string().contains("token"),
            "the refusal names the shape: {token}"
        );
    }

    #[test]
    fn a_service_account_key_is_accepted() {
        let credentials = Credentials::from_json(
            r#"{
                "id": "ajekey",
                "service_account_id": "ajeservice",
                "private_key": "a pem nobody uses"
            }"#,
        )
        .expect("a service-account key is a supported shape");
        assert_eq!(credentials.service_account_key().id, "ajekey");
    }

    #[test]
    fn no_debug_prints_the_key() {
        let credentials = Credentials::from_json(
            r#"{
                "id": "ajekey",
                "service_account_id": "ajeservice",
                "private_key": "a pem nobody uses"
            }"#,
        )
        .expect("a service-account key is a supported shape");
        let debug = format!("{credentials:?}");
        assert!(
            !debug.contains("a pem nobody uses"),
            "the key is redacted: {debug}"
        );
    }
}
