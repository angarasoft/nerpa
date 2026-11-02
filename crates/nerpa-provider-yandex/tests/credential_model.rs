//! The credential model adr-2026-079 asks for.
//!
//! A long-lived credential is refused by name, a service-account key is signed
//! for an IAM token the endpoint accepts, a key file open to others is refused,
//! a key is read for a signature and not kept, and no `Debug` output prints key
//! or token material.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
#![cfg(unix)]

use std::collections::BTreeMap;
use std::fs::Permissions;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use base64::Engine as _;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use nerpa_core::address::address;
use nerpa_core::observed::Observed;
use nerpa_core::provider::{Machines, Provider, Read, Reading};
use nerpa_provider_yandex::{
    Api, ApiError, Auth, AuthError, KeyError, Response, ServiceAccountKey, Yandex,
};
use ring::signature::{RSA_PSS_2048_8192_SHA256, RsaKeyPair, UnparsedPublicKey};

/// A fixed PKCS#8 key made for tests only: the private half signs, the public
/// half verifies, and neither is a credential anybody uses.
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

/// A 1024-bit key, made for tests only: too short to be accepted.
const WEAK_PEM: &str = "\
-----BEGIN PRIVATE KEY-----
MIICdgIBADANBgkqhkiG9w0BAQEFAASCAmAwggJcAgEAAoGBAM/KjK7c+VWrqc5I
QpdeirvjJA2Ir366r54jzfwoc/J/oI6r9t0n6UABVIXvV/jKHfjpBkABvWr+9Z5p
FLWW2spTxwwi1GMZcpXcaxuRHaD43ku4KwiGd08kwN8YgMsSnouDuHp8Lt93vxGA
an+VJMvSXgJZvy9+y1+zs7a1GJsTAgMBAAECgYAaEmEKg8FpJWvd7SUl1v/zSTG5
3P+gGA52+ikbTXwW3jqoDB2ibCKRHFmdoIVaOwn9tEdLeqxoo2rwIGEv06jJ5kEY
x9JPvxYD2askA7LgdaF9HiF+yjQlDlFjXRI2+WTF70jYVwK1QirqeoB40E7GtHMl
HNpeoSuCOMdvKkBC8QJBAPCZ1gYeCfedAhc7BOOVlByYQKVHh7NAji+p+esU3EjY
Q28qdNV8ZL4sErDa83qealMJqpEC+I8P9AW8p/LrxqkCQQDdFyM3q38Z3zL1gVp9
H9CSndAPGkRoJv4mG9rrrv12sMpF9+oSkZhtywNfzDbO6ugDNcjjLrwiYT/mE3Oy
6DVbAkAyJS3ZKouTQvL7mn5jXHp5FzZp/CWJM4wu3PNMjpV2zTQIyEV/dSSeCht/
JrMWPEwWsG4PORpK3t1SEMbEN2W5AkEAsTDV/Mvpdaa4hKJDD3zsAFWSZkg7Au4d
ShWRAsqgQPu3YRQr+O3PKE9ISQK1+mexwsDcW5cdGMCwIYj9dSk9eQJASOLFaBTX
vXAR47Z3hRTFMLC9fF+JQtUSpPfEcqV4oOMmpmMQ33sZ8g6ke6L0ft83HfZOPinh
aZSlCi1CddnUsw==
-----END PRIVATE KEY-----
";

/// The endpoint a JWT is exchanged at, and the audience it is signed for.
const IAM_TOKENS_URL: &str = "https://iam.api.cloud.yandex.net/iam/v1/tokens";

/// Writes a key file the way `yc iam key create` does, with `mode`.
fn key_file(dir: &Path, private_key: &str, mode: u32) -> PathBuf {
    let document = serde_json::json!({
        "id": "ajekey",
        "service_account_id": "ajeservice",
        "created_at": "2026-11-02T00:00:00Z",
        "key_algorithm": "RSA_2048",
        "public_key": "-----BEGIN PUBLIC KEY-----\nnot read\n-----END PUBLIC KEY-----\n",
        "private_key": format!("PLEASE DO NOT REMOVE THIS LINE!\n{private_key}"),
    });
    let path = dir.join(format!("key-{mode:o}.json"));
    std::fs::write(&path, document.to_string()).unwrap();
    std::fs::set_permissions(&path, Permissions::from_mode(mode)).unwrap();
    path
}

/// A Yandex Cloud that answers the IAM exchange and an empty list, and keeps
/// what it was asked.
#[derive(Default)]
struct Cloud {
    posts: Mutex<Vec<(String, String)>>,
    authorizations: Mutex<Vec<String>>,
}

/// The cloud as the provider's `Api`, shared so the test can look afterwards.
struct Shared(Arc<Cloud>);

impl Api for Shared {
    fn get(&self, _url: &str, headers: &[(&str, &str)]) -> Result<Response, ApiError> {
        let authorization = headers
            .iter()
            .find(|(name, _)| *name == "Authorization")
            .map_or_else(String::new, |(_, value)| (*value).to_owned());
        self.0.authorizations.lock().unwrap().push(authorization);
        Ok(Response {
            status: 200,
            body: r#"{"instances":[]}"#.to_owned(),
        })
    }

    fn post(&self, url: &str, _headers: &[(&str, &str)], body: &str) -> Result<Response, ApiError> {
        self.0
            .posts
            .lock()
            .unwrap()
            .push((url.to_owned(), body.to_owned()));
        Ok(Response {
            status: 200,
            body: r#"{"iamToken":"t1.fake","expiresAt":"2999-01-01T00:00:00Z"}"#.to_owned(),
        })
    }

    fn patch(
        &self,
        _url: &str,
        _headers: &[(&str, &str)],
        _body: &str,
    ) -> Result<Response, ApiError> {
        Err(ApiError::Transport("not asked in this test".to_owned()))
    }

    fn delete(&self, _url: &str, _headers: &[(&str, &str)]) -> Result<Response, ApiError> {
        Err(ApiError::Transport("not asked in this test".to_owned()))
    }
}

/// A provider over `cloud` that holds `auth`.
fn provider(auth: Auth, cloud: &Arc<Cloud>) -> Yandex {
    Yandex::new(
        "folder",
        "https://compute.example/compute/v1",
        "https://vpc.example/vpc/v1",
        auth,
        Box::new(Shared(Arc::clone(cloud))),
    )
}

/// Reads one instance through the provider.
fn read(provider: &Yandex) -> Result<Read, nerpa_core::provider::ProviderError> {
    let observed = Observed::new();
    let facts = observed.facts_of(None);
    let machines = Machines::new();
    let reach = machines
        .reach(None)
        .expect("a control resource has nowhere");
    provider.read(
        &[Reading::new(address("yandex:instance.db"), BTreeMap::new())],
        facts,
        reach,
    )
}

#[test]
fn a_long_lived_credential_is_refused_by_name() {
    for (document, name, value) in [
        (
            r#"{"api_key": "AQVN-api-value"}"#,
            "api_key",
            "AQVN-api-value",
        ),
        (
            r#"{"secret": "AQVN-secret-value"}"#,
            "secret",
            "AQVN-secret-value",
        ),
        (r#"{"token": "t1.token-value"}"#, "token", "t1.token-value"),
        (
            r#"{"static_token": "static-value"}"#,
            "static_token",
            "static-value",
        ),
        (
            r#"{"oauth_token": "y0_oauth-value"}"#,
            "oauth_token",
            "y0_oauth-value",
        ),
        // The same names in the other spellings a document may use.
        (
            r#"{"API_KEY": "AQVN-upper-value"}"#,
            "api_key",
            "AQVN-upper-value",
        ),
        (
            r#"{"apiKey": "AQVN-camel-value"}"#,
            "api_key",
            "AQVN-camel-value",
        ),
        (
            r#"{"Token": "t1.titled-value"}"#,
            "token",
            "t1.titled-value",
        ),
        (
            r#"{"secret-key": "dashed-value"}"#,
            "secret_key",
            "dashed-value",
        ),
        (
            r#"{"accessKey": "access-value"}"#,
            "access_key",
            "access-value",
        ),
    ] {
        let error = ServiceAccountKey::from_json(document).unwrap_err();
        assert!(
            matches!(error, KeyError::LongLived { name: refused } if refused == name),
            "{name}: {error}"
        );
        let message = error.to_string();
        assert!(
            message.contains(name),
            "the refusal names the shape: {message}"
        );
        assert!(
            !message.contains(value),
            "the refusal does not repeat the credential: {message}"
        );
    }

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("api-key.json");
    std::fs::write(&path, r#"{"api_key": "AQVN-api-value"}"#).unwrap();
    std::fs::set_permissions(&path, Permissions::from_mode(0o600)).unwrap();
    let error = Auth::service_account_key_file(&path).unwrap_err();
    assert!(
        matches!(
            error,
            AuthError::Key(KeyError::LongLived { name: "api_key" })
        ),
        "a key file that holds an API key is refused when it is named: {error}"
    );
}

#[test]
fn a_document_that_is_not_a_key_is_refused_without_repeating_it() {
    let pasted = ServiceAccountKey::from_json(r#""AQVN-pasted-credential""#).unwrap_err();
    assert!(matches!(pasted, KeyError::NotJson(_)), "{pasted}");
    assert!(
        !pasted.to_string().contains("AQVN-pasted-credential"),
        "{pasted}"
    );

    let wrong_shape = ServiceAccountKey::from_json(
        r#"{"id": "k", "service_account_id": 5, "private_key": "AQVN-wrong-shape"}"#,
    )
    .unwrap_err();
    assert!(matches!(wrong_shape, KeyError::NotAKey(_)), "{wrong_shape}");
    assert!(
        !wrong_shape.to_string().contains("AQVN-wrong-shape"),
        "{wrong_shape}"
    );
}

#[test]
fn a_key_file_that_others_can_read_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    for mode in [0o644, 0o640, 0o604, 0o666] {
        let path = key_file(dir.path(), KEY_PEM, mode);
        let error = Auth::service_account_key_file(&path).unwrap_err();
        assert!(
            matches!(error, AuthError::Key(KeyError::TooOpen { .. })),
            "mode {mode:o}: {error}"
        );
        let message = error.to_string();
        assert!(message.contains("readable by group or others"), "{message}");
        assert!(message.contains(&format!("{mode:03o}")), "{message}");
        assert!(message.contains(path.to_str().unwrap()), "{message}");
    }
    for mode in [0o600, 0o400] {
        let path = key_file(dir.path(), KEY_PEM, mode);
        Auth::service_account_key_file(&path).expect("a key file only its owner reads is accepted");
    }
}

#[test]
fn a_key_under_2048_bits_is_refused_when_it_is_named() {
    let dir = tempfile::tempdir().unwrap();
    let path = key_file(dir.path(), WEAK_PEM, 0o600);
    let error = Auth::service_account_key_file(&path).unwrap_err();
    assert!(
        matches!(error, AuthError::Key(KeyError::BadKey(_))),
        "{error}"
    );
    // ring's own reason, not nerpa's wording: a corrupt PEM is refused too, and
    // must not be mistaken for this.
    assert!(error.to_string().contains("TooSmall"), "{error}");
}

#[test]
fn a_key_that_is_not_a_regular_file_is_refused_and_not_waited_on() {
    let dir = tempfile::tempdir().unwrap();
    let fifo = dir.path().join("key.fifo");
    if !std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .is_ok_and(|status| status.success())
    {
        return; // no mkfifo here, so there is no FIFO to name
    }
    std::fs::set_permissions(&fifo, Permissions::from_mode(0o600)).unwrap();

    // A FIFO with no writer blocks an open for good, so a refusal that came
    // back at all came back without opening it.
    let error = Auth::service_account_key_file(&fifo).unwrap_err();
    assert!(
        matches!(error, AuthError::Key(KeyError::NotAFile { .. })),
        "{error}"
    );

    let directory = Auth::service_account_key_file(dir.path()).unwrap_err();
    assert!(
        matches!(directory, AuthError::Key(KeyError::NotAFile { .. })),
        "{directory}"
    );
}

#[test]
fn a_key_file_larger_than_a_key_can_be_is_refused_unread() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("huge.json");
    std::fs::write(&path, vec![b' '; 70 * 1024]).unwrap();
    std::fs::set_permissions(&path, Permissions::from_mode(0o600)).unwrap();
    let error = Auth::service_account_key_file(&path).unwrap_err();
    assert!(
        matches!(error, AuthError::Key(KeyError::TooLarge { .. })),
        "{error}"
    );
}

#[test]
fn a_service_account_key_is_signed_for_an_exchange_the_endpoint_accepts() {
    let dir = tempfile::tempdir().unwrap();
    let path = key_file(dir.path(), KEY_PEM, 0o600);
    let cloud = Arc::new(Cloud::default());
    let provider = provider(Auth::service_account_key_file(&path).unwrap(), &cloud);

    read(&provider).expect("the read goes through");

    let posts = cloud.posts.lock().unwrap();
    assert_eq!(posts.len(), 1, "one exchange, and nothing else posted");
    let (url, body) = &posts[0];
    assert_eq!(url, IAM_TOKENS_URL);
    let body: serde_json::Value = serde_json::from_str(body).unwrap();
    let jwt = body["jwt"].as_str().expect("the exchange carries a jwt");

    let mut parts = jwt.split('.');
    let header_part = parts.next().unwrap();
    let claims = parts.next().unwrap();
    let signature = parts.next().unwrap();
    assert!(parts.next().is_none(), "exactly three parts");

    let header: serde_json::Value =
        serde_json::from_slice(&URL_SAFE_NO_PAD.decode(header_part).unwrap()).unwrap();
    assert_eq!(header["alg"], "PS256", "the endpoint accepts only PS256");
    assert_eq!(header["typ"], "JWT");
    assert_eq!(header["kid"], "ajekey");

    let claims_json: serde_json::Value =
        serde_json::from_slice(&URL_SAFE_NO_PAD.decode(claims).unwrap()).unwrap();
    assert_eq!(claims_json["iss"], "ajeservice");
    assert_eq!(claims_json["aud"], IAM_TOKENS_URL);
    let lifetime = claims_json["exp"].as_u64().unwrap() - claims_json["iat"].as_u64().unwrap();
    assert!(
        (1..=3_600).contains(&lifetime),
        "the endpoint allows an hour at most, and a JWT is used at once: {lifetime}"
    );

    // The signature holds under the key's public half, taken from the same PEM.
    let body_of_pem: String = KEY_PEM
        .lines()
        .filter(|line| !line.starts_with("-----"))
        .collect();
    let der = STANDARD.decode(body_of_pem).unwrap();
    let public = RsaKeyPair::from_pkcs8(&der)
        .unwrap()
        .public()
        .as_ref()
        .to_vec();
    UnparsedPublicKey::new(&RSA_PSS_2048_8192_SHA256, public)
        .verify(
            format!("{header_part}.{claims}").as_bytes(),
            &URL_SAFE_NO_PAD.decode(signature).unwrap(),
        )
        .expect("the signature is a PS256 signature of this key");

    // What the exchange answered is what the next call carries.
    assert_eq!(
        *cloud.authorizations.lock().unwrap(),
        vec!["Bearer t1.fake".to_owned()]
    );
}

#[test]
fn a_key_is_read_for_a_signature_and_not_kept() {
    let dir = tempfile::tempdir().unwrap();
    let path = key_file(dir.path(), KEY_PEM, 0o600);
    let cloud = Arc::new(Cloud::default());
    let provider = provider(Auth::service_account_key_file(&path).unwrap(), &cloud);

    std::fs::remove_file(&path).unwrap();

    let error = read(&provider).unwrap_err();
    let message = error.to_string();
    assert!(
        message.contains(path.to_str().unwrap()),
        "the exchange looked for the key where it was named: {message}"
    );
    assert!(
        cloud.posts.lock().unwrap().is_empty(),
        "no key was kept to sign with, so nothing was exchanged"
    );
}

#[test]
fn no_debug_output_prints_key_or_token_material() {
    let dir = tempfile::tempdir().unwrap();
    let path = key_file(dir.path(), KEY_PEM, 0o600);
    let cloud = Arc::new(Cloud::default());

    let key = ServiceAccountKey::from_json(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let static_token = Auth::IamToken("t1.secret-token-text".to_owned());
    let static_token_debug = format!("{static_token:?}");
    let by_key = Auth::service_account_key_file(&path).unwrap();
    let by_key_debug = format!("{by_key:?}");
    let provider_debug = format!(
        "{:?}",
        provider(Auth::IamToken("t1.in-a-provider".to_owned()), &cloud)
    );

    for (what, text) in [
        ("a key", format!("{key:?}")),
        ("a static token", static_token_debug),
        ("a key file", by_key_debug),
        ("a provider", provider_debug),
    ] {
        for material in ["MIIE", "PRIVATE KEY", "secret-token-text", "in-a-provider"] {
            assert!(
                !text.contains(material),
                "{what} prints {material:?}: {text}"
            );
        }
    }
}
