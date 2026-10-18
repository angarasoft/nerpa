//! The exchange's contract against the endpoint's documented requirements.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use nerpa_provider_yandex::auth::{ServiceAccountKey, signed_jwt};

/// A fixed PKCS#8 key made for tests only: it signs test JWTs, and is not a
/// credential anybody uses.
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

#[test]
fn a_signed_jwt_carries_the_algorithm_the_endpoint_accepts() {
    let key = ServiceAccountKey {
        id: "ajekey".to_owned(),
        service_account_id: "ajeservice".to_owned(),
        private_key: KEY_PEM.to_owned(),
    };
    let jwt = signed_jwt(&key, 1_700_000_000, 3_600).expect("signed");

    let header = jwt.split('.').next().expect("a header");
    let decoded = URL_SAFE_NO_PAD.decode(header).expect("base64");
    let header: serde_json::Value = serde_json::from_slice(&decoded).expect("json");
    assert_eq!(header["alg"], "PS256", "the endpoint accepts only PS256");
    assert_eq!(header["typ"], "JWT");
    assert_eq!(header["kid"], "ajekey");
}
