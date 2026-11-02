//! The credential the fleet names is checked when the engine is assembled, not
//! at the first apply (adr-2026-079).

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
#![cfg(unix)]

use std::fs::Permissions;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use nerpa_config::fleet::Fleet;
use nerpa_engine::{Engine, EngineError};

/// A fixed PKCS#8 key made for tests only: it signs nothing anybody uses.
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

/// Writes `content` as a file with `mode`.
fn file(dir: &Path, name: &str, content: &str, mode: u32) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, content).unwrap();
    std::fs::set_permissions(&path, Permissions::from_mode(mode)).unwrap();
    path
}

/// A key as `yc iam key create` writes it down. Written by hand, so this crate
/// needs no JSON dependency for one test: the PEM's newlines are the only
/// characters that need escaping.
fn a_key_document() -> String {
    let escaped = KEY_PEM.replace('\n', "\\n");
    format!(
        "{{\"id\": \"ajekey\", \"service_account_id\": \"ajeservice\", \
         \"private_key\": \"PLEASE DO NOT REMOVE THIS LINE!\\n{escaped}\"}}"
    )
}

/// The engine assembled over a fleet whose cloud names `key_file`.
fn assemble(dir: &Path, key_file: &Path) -> Result<Engine, EngineError> {
    let fleet = Fleet::read(&format!(
        "[cloud.yandex]\nfolder_id = \"b1g\"\nkey_file = \"{}\"\n",
        key_file.display()
    ))
    .expect("a fleet");
    Engine::assemble("main.star", "", dir, &fleet)
}

#[test]
fn a_key_file_that_others_can_read_stops_the_engine_before_anything_runs() {
    let dir = tempfile::tempdir().unwrap();
    let key = file(dir.path(), "key.json", &a_key_document(), 0o644);

    let error = assemble(dir.path(), &key).unwrap_err();

    assert!(matches!(error, EngineError::Credential(_)), "{error}");
    assert!(
        error.to_string().contains("readable by group or others"),
        "{error}"
    );
}

#[test]
fn a_long_lived_credential_in_the_key_file_stops_the_engine_by_name() {
    let dir = tempfile::tempdir().unwrap();
    let key = file(
        dir.path(),
        "key.json",
        r#"{"api_key": "AQVN-api-value"}"#,
        0o600,
    );

    let error = assemble(dir.path(), &key).unwrap_err();

    assert!(matches!(error, EngineError::Credential(_)), "{error}");
    let message = error.to_string();
    assert!(message.contains("api_key"), "{message}");
    assert!(!message.contains("AQVN-api-value"), "{message}");
}

#[test]
fn a_key_file_only_its_owner_reads_assembles() {
    let dir = tempfile::tempdir().unwrap();
    let key = file(dir.path(), "key.json", &a_key_document(), 0o600);

    assemble(dir.path(), &key).expect("a usable key file assembles an engine");
}
