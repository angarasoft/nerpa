//! How the provider proves who it is to Yandex Cloud.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use zeroize::Zeroizing;

use crate::api::{Api, ApiError};
use crate::key::{KeyError, place};
use crate::signer::{RingSigner, Signer};

/// The IAM service's token endpoint: where an OAuth token or a signed JWT is
/// exchanged for an IAM token, and the `aud` the JWT is signed for.
const IAM_TOKENS_URL: &str = "https://iam.api.cloud.yandex.net/iam/v1/tokens";

/// How long a signed JWT is valid. It is used once, at once, so it is short; the
/// endpoint allows an hour at most.
const JWT_TTL_SECONDS: u64 = 600;

/// How long before its expiry a token is no longer reused, so a call made late in
/// a run does not meet a token that stops working under it (adr-2026-079).
const REFRESH_MARGIN: Duration = Duration::from_secs(60);

/// A credential, resolved into an IAM token when a call needs one.
///
/// The `Debug` output names the shape and prints no token and no key
/// (adr-2026-079). The enum is non-exhaustive so it can grow without breaking
/// this crate's users.
#[non_exhaustive]
pub enum Auth {
    /// A static IAM token, used as-is until it expires. It is short-lived, which
    /// is what makes it fit for CI; it is not the long-lived token the model
    /// refuses.
    IamToken(String),
    /// The token is read from the instance metadata service.
    Metadata,
    /// The yc CLI profile: its OAuth token is exchanged for an IAM token
    /// (adr-2026-071).
    YcProfile,
    /// A service-account key: a JWT it signs is exchanged for an IAM token
    /// (adr-2026-079). The key never enters this value; the signer holds where
    /// it is, and reads it only for a signature.
    ServiceAccountKey(Box<dyn Signer>),
}

impl std::fmt::Debug for Auth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Auth::IamToken(_) => f.write_str("IamToken(<redacted>)"),
            Auth::Metadata => f.write_str("Metadata"),
            Auth::YcProfile => f.write_str("YcProfile"),
            Auth::ServiceAccountKey(signer) => {
                f.debug_tuple("ServiceAccountKey").field(signer).finish()
            }
        }
    }
}

impl Auth {
    /// A credential that is the service-account key in the file at `path`.
    ///
    /// The file is read now, so that one readable by others, one that holds a
    /// long-lived credential, or a key that cannot sign is refused when the fleet
    /// is read and not at the first apply.
    ///
    /// # Errors
    ///
    /// Returns [`AuthError::Key`] when the file cannot be used as a key.
    pub fn service_account_key_file(path: impl Into<PathBuf>) -> Result<Self, AuthError> {
        Ok(Self::ServiceAccountKey(Box::new(RingSigner::new(path)?)))
    }
}

/// Why a token could not be made from a service-account key.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AuthError {
    /// The key file cannot be used as a key.
    #[error("the service-account key is not usable: {0}")]
    Key(#[from] KeyError),
    /// The signature could not be made.
    #[error("the JWT could not be signed")]
    Sign,
    /// The clock reads before the epoch, or the JWT's lifetime overflows.
    #[error("the clock does not allow a JWT: it reads before the epoch, or the lifetime overflows")]
    Clock,
    /// A part of the JWT could not be encoded.
    #[error("a part of the JWT could not be encoded: {0}")]
    Encode(String),
}

impl From<AuthError> for ApiError {
    fn from(error: AuthError) -> Self {
        ApiError::Transport(error.to_string())
    }
}

/// A resolved token and, where the source says so, when it expires.
///
/// The `Debug` output redacts the token (adr-2026-079).
pub(crate) struct Resolved {
    token: String,
    expires_at: Option<SystemTime>,
}

impl std::fmt::Debug for Resolved {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Resolved")
            .field("token", &"<redacted>")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

impl Resolved {
    /// Whether the token can still be used: it has no expiry, or its expiry is
    /// further off than the margin a late call needs.
    fn fresh(&self) -> bool {
        self.expires_at
            .is_none_or(|expires_at| SystemTime::now() + REFRESH_MARGIN < expires_at)
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
                let token: TokenAnswer = serde_json::from_str(&answer.body).map_err(|error| {
                    ApiError::Transport(format!(
                        "the metadata answer is not a token ({})",
                        place(&error)
                    ))
                })?;
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
            Auth::ServiceAccountKey(signer) => {
                let jwt =
                    Zeroizing::new(signer.jwt(IAM_TOKENS_URL, now_seconds()?, JWT_TTL_SECONDS)?);
                let (token, expires_at) = exchange_jwt(api, &jwt)?;
                Ok(Resolved {
                    token,
                    expires_at: Some(expires_at),
                })
            }
        }
    }
}

/// The wall clock now, in seconds since the epoch.
fn now_seconds() -> Result<u64, AuthError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .map_err(|_| AuthError::Clock)
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

/// The body that exchanges an OAuth token for an IAM token.
#[derive(serde::Serialize)]
struct OauthRequest<'a> {
    #[serde(rename = "yandexPassportOauthToken")]
    oauth_token: &'a str,
}

/// The body that exchanges a signed JWT for an IAM token.
#[derive(serde::Serialize)]
struct JwtRequest<'a> {
    jwt: &'a str,
}

/// Exchanges an OAuth token for an IAM token and its expiry, over the IAM API.
fn exchange_oauth(api: &dyn Api, oauth: &str) -> Result<(String, SystemTime), ApiError> {
    send(api, &OauthRequest { oauth_token: oauth }, oauth)
}

/// Exchanges a signed JWT for an IAM token and its expiry, over the IAM API.
fn exchange_jwt(api: &dyn Api, jwt: &str) -> Result<(String, SystemTime), ApiError> {
    send(api, &JwtRequest { jwt }, jwt)
}

/// Writes `request`, which carries `credential`, straight into a buffer that is
/// wiped when it is dropped, and exchanges it.
///
/// The body goes through no intermediate JSON value, which would hold a copy of
/// the credential that nothing wipes, and the buffer is sized for the credential
/// at its most escaped so it is not grown by copying.
fn send<T: serde::Serialize>(
    api: &dyn Api,
    request: &T,
    credential: &str,
) -> Result<(String, SystemTime), ApiError> {
    let mut body = Zeroizing::new(Vec::with_capacity(
        credential.len().saturating_mul(6).saturating_add(64),
    ));
    serde_json::to_writer(&mut *body, request).map_err(|error| {
        ApiError::Transport(format!(
            "the exchange body could not be written ({})",
            place(&error)
        ))
    })?;
    let text = std::str::from_utf8(&body)
        .map_err(|_| ApiError::Transport("the exchange body is not text".to_owned()))?;
    exchange(api, text)
}

/// One exchange at the IAM token endpoint.
///
/// A refusal names the status and nothing of the answer: the endpoint is the one
/// place a credential has just been sent to, and its error text is not echoed.
fn exchange(api: &dyn Api, body: &str) -> Result<(String, SystemTime), ApiError> {
    let answer = api.post(IAM_TOKENS_URL, &[], body)?;
    if answer.status != 200 {
        return Err(ApiError::Transport(format!(
            "the IAM service answered {} to the token exchange",
            answer.status
        )));
    }
    let token: IamTokenAnswer = serde_json::from_str(&answer.body).map_err(|error| {
        ApiError::Transport(format!("the IAM answer is not a token ({})", place(&error)))
    })?;
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
        status: u16,
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
                status: self.status,
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
            status: 200,
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
    fn a_refused_exchange_names_the_status_and_nothing_of_the_answer() {
        let api = Replay {
            status: 401,
            body: r#"{"message":"echoed-credential-text"}"#,
        };
        let problem = exchange_jwt(&api, "a.jwt.here").unwrap_err().to_string();
        assert!(problem.contains("401"), "{problem}");
        assert!(!problem.contains("echoed-credential-text"), "{problem}");
    }

    #[test]
    fn an_answer_that_is_not_a_token_is_refused_without_quoting_it() {
        // A bare string where an object belongs: the shape serde's own message
        // would quote back.
        let api = Replay {
            status: 200,
            body: r#""quoted-token-text""#,
        };
        let problem = exchange_jwt(&api, "a.jwt.here").unwrap_err().to_string();
        assert!(!problem.contains("quoted-token-text"), "{problem}");
    }

    #[test]
    fn a_token_about_to_expire_is_resolved_again() {
        let cache = TokenCache::default();
        let nearly_spent = || {
            Ok(Resolved {
                token: "t1".to_owned(),
                expires_at: Some(SystemTime::now() + Duration::from_secs(30)),
            })
        };
        assert_eq!(cache.token(nearly_spent).unwrap(), "t1");
        assert_eq!(
            cache
                .token(|| {
                    Ok(Resolved {
                        token: "t2".to_owned(),
                        expires_at: Some(SystemTime::now() + Duration::from_secs(3600)),
                    })
                })
                .unwrap(),
            "t2",
            "a token with less than the margin left is not reused"
        );
    }

    #[test]
    fn a_resolved_token_is_not_printed() {
        let resolved = Resolved {
            token: "t1.secret-token-text".to_owned(),
            expires_at: None,
        };
        let text = format!("{resolved:?} {:?}", TokenCache::default());
        assert!(!text.contains("secret-token-text"), "{text}");
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
