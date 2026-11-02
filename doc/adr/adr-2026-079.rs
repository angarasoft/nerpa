use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "The Yandex service-account key is signed by ring behind a seam, and the wipe is stated for what nerpa holds",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    subsystems: &[taxon!(Subsystem, Yandex), taxon!(Subsystem, Config), taxon!(Subsystem, Core)],
    context: r"
        adr-2026-075 names the credential model: a long-lived service-account
        key enters the flow, a short-lived IAM token leaves it, and the key is
        zeroized once the JWT is signed. It names no signer, and the REST port
        of the key flow (w-yc-credential-model) needs one: Yandex accepts only
        PS256, RSASSA-PSS with SHA-256.

        What the tree and the advisory database say on 2026-11-02. The
        RustCrypto rsa crate carries RUSTSEC-2023-0071, a timing side channel
        that can leak the private key, with no patched version: 0.9.10 and
        0.10.0-rc.18 are both affected. ring 0.17.14 is already linked through
        rustls, which ureq uses; its 'unmaintained' advisory RUSTSEC-2025-0007
        was withdrawn on 2025-02-22. ring signs RSA-PSS with SHA-256 and the
        develop line signed with it. Its source holds no zeroization of an RSA
        key: RsaKeyPair has no Drop, so the parsed key stays in freed memory.
        aws-lc-rs 1.19.0 is a ring-compatible alternative whose FIPS build
        carries a licence, OpenSSL, that deny.toml does not allow, and which
        links a second cryptographic provider beside rustls's.

        Yandex also offers paths that hold no key at all: the instance
        metadata service, which the provider already reads, and workload
        identity federation, which exchanges the OIDC token of a CI job for an
        IAM token.
    ",
    decision: r"
        adr-2026-075 is superseded. Its model stands: the carried credential is
        a short-lived IAM token refreshed before it expires, with the expiry
        parsed rather than carried as text; a long-lived credential as the
        carried mode is refused by name; no Debug output prints key or token
        material; iam_token stays for CI because it is short-lived.

        Signing. The JWT is signed PS256 by ring, behind a Signer trait in the
        provider crate that yields the signed JWT. The exchange depends on the
        trait, not on ring, so aws-lc-rs or a signer outside the process
        replaces ring by implementing one trait. The rsa crate is not admitted
        while RUSTSEC-2023-0071 has no patch.

        The key file. The key is read from a file the fleet names, never from
        an environment variable, a command-line argument or the fleet document
        itself. A file that group or others can read is refused. The file is
        read when a token is needed and the key is not kept between exchanges;
        a key under 2048 bits is refused.

        What is wiped. Every buffer nerpa owns that held key material — the
        file contents, the PEM text and the decoded DER — is zeroized when
        signing ends, on success and on error. Copies that other code makes are
        not wiped: ring does not wipe its parsed key and the JSON parser keeps
        a scratch buffer. The limitation l-yc-key-copies-unwiped declares that
        gap; it closes only when the signer changes.

        Keyless first. The metadata service is the path on a machine inside
        Yandex Cloud. Workload identity federation is the intended path for a
        CI run, so that it holds no key at all; it is in the backlog as
        w-yc-wlif and is not built. The key is the fallback.
    ",
    trade_offs: &[
        "ring does not wipe its own copy of the parsed key and has no FIPS build. If ring stalls on a vulnerability, the seam lets aws-lc-rs replace it, at the price of a C build and a second cryptographic provider.",
        "Refusing a key file that group or others can read costs a chmod for a key checked out of a shared directory; the refusal names the file and the mode.",
        "Reading the key for each exchange costs a file read per token, which is once per run for a token that lives hours; it buys that no key sits in memory between runs of the exchange.",
    ],
    constraints: &[
        "A signer whose crate carries an unpatched advisory is not admitted; rsa is out while RUSTSEC-2023-0071 has no patch.",
        "A key file that group or others can read is refused by name, held by a test.",
        "A long-lived credential shape in a key file or in the fleet document is refused by name, held by a test. The executors are w-yc-credential-model for the key file and w-yc-key-file for the fleet document.",
        "Buffers nerpa owns that held key material are zeroized; the claim does not reach copies inside ring or the JSON parser, and l-yc-key-copies-unwiped says so.",
    ],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 11, 2),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[crate::rfc::rfc_2026_002],
    introduces_norm: true,
);
