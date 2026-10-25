//! The exchange's contract against the endpoint's documented requirements.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

mod common;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use nerpa_provider_yandex::auth::signed_jwt;

#[test]
fn a_signed_jwt_carries_the_algorithm_the_endpoint_accepts() {
    let jwt = signed_jwt(&common::a_key(), 1_700_000_000, 3_600).expect("signed");

    let header = jwt.split('.').next().expect("a header");
    let decoded = URL_SAFE_NO_PAD.decode(header).expect("base64");
    let header: serde_json::Value = serde_json::from_slice(&decoded).expect("json");
    assert_eq!(header["alg"], "PS256", "the endpoint accepts only PS256");
    assert_eq!(header["typ"], "JWT");
    assert_eq!(header["kid"], "ajekey");
}
