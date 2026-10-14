use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "A long-lived key enters the Yandex flow; a short-lived token leaves it",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    subsystems: &[taxon!(Subsystem, Yandex), taxon!(Subsystem, Core)],
    context: r"
        The landed Yandex client claims a long-lived key is never a mode, but the
        flow begins with one: a service-account key is the long-lived input the
        IAM token is minted from, and the exchange cannot be avoided — it is the
        mechanism Yandex offers. The claim lived in a work's justification
        string, held by no code, which ADR-069 refuses; i0030's rationale says
        no code path reads a long-lived key, which the client falsifies. An
        independent review called both out.
    ",
    decision: r"
        The model is named as it is. The service-account key is the long-lived
        input: it enters from a secret source, is held in memory only to sign
        one JWT, and is zeroized after signing. The credential the provider
        carries onwards is the short-lived IAM token, refreshed before its
        expiry, which is parsed rather than carried as a string. A long-lived
        credential as the carried mode — an API key, a static token, or the key
        itself on the API path — is refused by default. Key and token material
        is never printed by Debug. i0030's rationale is scoped to actor
        identity: a provider's service-account key is a cloud credential, not an
        identity, and INV-TRUST-001 is unchanged.
    ",
    trade_offs: &[r"
        The key is still long-lived and still held by whoever stores it: the
        exchange narrows the window in which a leaked credential works to the
        token's lifetime; it does not remove the key from the world. Refusing
        long-lived modes costs flexibility — every supported credential shape
        must route through the exchange — which is the point.
    "],
    constraints: &[
        "A credential config that is long-lived-shaped — an API key or a static token — is refused by name, held by a test; the executor is w-yc-credentials-held, declared in the same change.",
        "The key's bytes are zeroized once the JWT is signed, and no Debug output prints key or token material.",
        "The token is refreshed before expires_at, which is parsed rather than carried as a string.",
    ],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 10, 15),
    breaking: Breaking::No,
    introduces_norm: true,
    code_refs: &[],
    related_rfcs: &[crate::rfc::rfc_2026_002],
);
