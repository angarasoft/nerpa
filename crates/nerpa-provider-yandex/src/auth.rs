//! How the provider proves who it is to Yandex Cloud.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use serde::Deserialize;

use crate::api::{Api, ApiError};

/// A credential, resolved into an IAM token when a call needs one.
///
/// A service-account key — the fourth shape — is not here yet: it exchanges a
/// signed JWT for a token, and the signing wants a dependency of its own. The
/// enum is non-exhaustive so it can grow without breaking this crate's users.
#[derive(Debug)]
#[non_exhaustive]
pub enum Auth {
    /// A static IAM token, used as-is until it expires.
    IamToken(String),
    /// The token is read from the instance metadata service.
    Metadata,
    /// The yc CLI profile: its OAuth token is exchanged for an IAM token
    /// (adr-2026-071).
    YcProfile,
}

/// A resolved token and, where the source says so, when it expires.
#[derive(Debug)]
pub(crate) struct Resolved {
    token: String,
    expires_at: Option<SystemTime>,
}

impl Resolved {
    /// Whether the token can still be used: it has no expiry, or its expiry has
    /// not passed.
    fn fresh(&self) -> bool {
        self.expires_at
            .is_none_or(|expires_at| SystemTime::now() < expires_at)
    }
}

/// A token remembered until it expires, so a run resolves once rather than once
/// per read and act.
#[derive(Debug, Default)]
pub(crate) struct TokenCache {
    resolved: Mutex<Option<Resolved>>,
}

impl TokenCache {
    /// The cached token if it is still fresh, else `resolve` and remember it.
    pub(crate) fn token(
        &self,
        resolve: impl FnOnce() -> Result<Resolved, ApiError>,
    ) -> Result<String, ApiError> {
        {
            let cached = self
                .resolved
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(resolved) = cached.as_ref()
                && resolved.fresh()
            {
                return Ok(resolved.token.clone());
            }
        }
        let resolved = resolve()?;
        let token = resolved.token.clone();
        *self
            .resolved
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(resolved);
        Ok(token)
    }
}

/// What the metadata service answers with.
#[derive(Deserialize)]
struct TokenAnswer {
    #[serde(rename = "access_token")]
    access_token: String,
    #[serde(rename = "expires_in")]
    expires_in: u64,
}

/// What an OAuth-for-IAM exchange answers with.
#[derive(Deserialize)]
struct IamTokenAnswer {
    #[serde(rename = "iamToken")]
    iam_token: String,
    #[serde(rename = "expiresAt")]
    expires_at: String,
}

/// The yc CLI configuration: the active profile and the profiles themselves.
#[derive(Deserialize)]
struct YcConfig {
    current: String,
    profiles: BTreeMap<String, Profile>,
}

/// One yc profile, of which only the OAuth token matters here.
#[derive(Deserialize)]
struct Profile {
    token: String,
}

impl Auth {
    /// The token for the `Authorization` header, and when it expires.
    pub(crate) fn resolve(&self, api: &dyn Api) -> Result<Resolved, ApiError> {
        match self {
            Auth::IamToken(token) => Ok(Resolved {
                token: token.clone(),
                expires_at: None,
            }),
            Auth::Metadata => {
                let answer = api.get(
                    "http://169.254.169.254/computeMetadata/v1/instance/service-accounts/default/token",
                    &[("Metadata-Flavor", "Google")],
                )?;
                let token: TokenAnswer = serde_json::from_str(&answer.body)
                    .map_err(|error| ApiError::Transport(error.to_string()))?;
                Ok(Resolved {
                    token: token.access_token,
                    expires_at: Some(SystemTime::now() + Duration::from_secs(token.expires_in)),
                })
            }
            Auth::YcProfile => {
                let oauth = yc_oauth_token()?;
                let (token, expires_at) = exchange_oauth(api, &oauth)?;
                Ok(Resolved {
                    token,
                    expires_at: Some(expires_at),
                })
            }
        }
    }
}

/// The OAuth token the active yc profile holds, from the yc config file.
fn yc_oauth_token() -> Result<String, ApiError> {
    let home = std::env::var("HOME")
        .map_err(|error| ApiError::Transport(format!("no HOME in the environment: {error}")))?;
    let path = Path::new(&home)
        .join(".config")
        .join("yandex-cloud")
        .join("config.yaml");
    yc_oauth_token_at(&path)
}

/// The OAuth token of the active profile in a yc config file at `path`.
fn yc_oauth_token_at(path: &Path) -> Result<String, ApiError> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| ApiError::Transport(format!("{}: {error}", path.display())))?;
    parse_yc_config(&text)
        .map_err(|problem| ApiError::Transport(format!("{}: {problem}", path.display())))
}

/// The OAuth token of the active profile in a yc config document.
fn parse_yc_config(text: &str) -> Result<String, String> {
    let config: YcConfig = serde_saphyr::from_str(text).map_err(|error| error.to_string())?;
    config
        .profiles
        .get(&config.current)
        .map(|profile| profile.token.clone())
        .ok_or_else(|| {
            format!(
                "the active profile \"{}\" is not among the profiles",
                config.current
            )
        })
}

/// Exchanges an OAuth token for an IAM token and its expiry, over the IAM API.
fn exchange_oauth(api: &dyn Api, oauth: &str) -> Result<(String, SystemTime), ApiError> {
    let body = serde_json::json!({ "yandexPassportOauthToken": oauth }).to_string();
    let answer = api.post("https://iam.api.cloud.yandex.net/iam/v1/tokens", &[], &body)?;
    let token: IamTokenAnswer = serde_json::from_str(&answer.body)
        .map_err(|error| ApiError::Transport(error.to_string()))?;
    let expires_at = parse_expiry(&token.expires_at)?;
    Ok((token.iam_token, expires_at))
}

/// The expiry of an IAM token, from the RFC 3339 the API answers with.
fn parse_expiry(text: &str) -> Result<SystemTime, ApiError> {
    let datetime = chrono::DateTime::parse_from_rfc3339(text)
        .map_err(|error| ApiError::Transport(format!("`expiresAt` is not RFC 3339: {error}")))?;
    Ok(datetime.with_timezone(&chrono::Utc).into())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::api::Response;

    /// An API that answers one recorded POST and nothing else.
    struct Replay {
        body: &'static str,
    }

    impl Api for Replay {
        fn get(&self, _url: &str, _headers: &[(&str, &str)]) -> Result<Response, ApiError> {
            unreachable!("only the exchange POST is exercised")
        }
        fn post(
            &self,
            _url: &str,
            _headers: &[(&str, &str)],
            _body: &str,
        ) -> Result<Response, ApiError> {
            Ok(Response {
                status: 200,
                body: self.body.to_owned(),
            })
        }
        fn patch(
            &self,
            _url: &str,
            _headers: &[(&str, &str)],
            _body: &str,
        ) -> Result<Response, ApiError> {
            unreachable!("only the exchange POST is exercised")
        }
        fn delete(&self, _url: &str, _headers: &[(&str, &str)]) -> Result<Response, ApiError> {
            unreachable!("only the exchange POST is exercised")
        }
    }

    #[test]
    fn the_active_profile_yields_its_oauth_token() {
        let config =
            "current: default\nprofiles:\n  default:\n    token: y0__token\n    cloud-id: b1g\n";
        assert_eq!(parse_yc_config(config).unwrap(), "y0__token");
    }

    #[test]
    fn a_missing_active_profile_is_named() {
        let config = "current: missing\nprofiles:\n  default:\n    token: y0__token\n";
        let problem = parse_yc_config(config).unwrap_err();
        assert!(problem.contains("missing"), "{problem}");
    }

    #[test]
    fn an_oauth_token_exchanges_for_an_iam_token() {
        let api = Replay {
            body: r#"{"iamToken":"t1","expiresAt":"2999-01-01T00:00:00Z"}"#,
        };
        let (token, expires_at) = exchange_oauth(&api, "oauth").unwrap();
        assert_eq!(token, "t1");
        assert!(
            expires_at > SystemTime::now(),
            "the exchange carries the token's expiry"
        );
    }

    #[test]
    fn a_fresh_token_is_reused_without_resolving_again() {
        let cache = TokenCache::default();
        let resolve = || {
            Ok(Resolved {
                token: "t1".to_owned(),
                expires_at: Some(SystemTime::now() + Duration::from_secs(3600)),
            })
        };
        assert_eq!(cache.token(resolve).unwrap(), "t1");
        assert_eq!(
            cache
                .token(|| panic!("a fresh token must not be resolved again"))
                .unwrap(),
            "t1"
        );
    }

    #[test]
    fn a_spent_token_is_resolved_again() {
        let cache = TokenCache::default();
        let resolve = || {
            Ok(Resolved {
                token: "t1".to_owned(),
                expires_at: Some(SystemTime::now() - Duration::from_secs(1)),
            })
        };
        assert_eq!(cache.token(resolve).unwrap(), "t1");
        assert_eq!(
            cache
                .token(|| {
                    Ok(Resolved {
                        token: "t2".to_owned(),
                        expires_at: Some(SystemTime::now() + Duration::from_secs(3600)),
                    })
                })
                .unwrap(),
            "t2"
        );
    }
}
