//! How the provider proves who it is to Yandex Cloud.

use serde::Deserialize;

use crate::api::{Api, ApiError};

/// A credential, resolved into an IAM token when a call needs one.
///
/// A service-account key — the third shape — is not here yet: it exchanges a
/// signed JWT for a token, and the signing wants a dependency of its own. The
/// enum is non-exhaustive so it can grow without breaking this crate's users.
#[derive(Debug)]
#[non_exhaustive]
pub enum Auth {
    /// A static IAM token, used as-is until it expires.
    IamToken(String),
    /// The token is read from the instance metadata service.
    Metadata,
}

/// What the metadata service answers with.
#[derive(Deserialize)]
struct TokenAnswer {
    #[serde(rename = "access_token")]
    access_token: String,
}

impl Auth {
    /// The token for the `Authorization` header.
    pub(crate) fn resolve(&self, api: &dyn Api) -> Result<String, ApiError> {
        match self {
            Auth::IamToken(token) => Ok(token.clone()),
            Auth::Metadata => {
                let answer = api.get(
                    "http://169.254.169.254/computeMetadata/v1/instance/service-accounts/default/token",
                    &[("Metadata-Flavor", "Google")],
                )?;
                let token: TokenAnswer = serde_json::from_str(&answer.body)
                    .map_err(|error| ApiError::Transport(error.to_string()))?;
                Ok(token.access_token)
            }
        }
    }
}
