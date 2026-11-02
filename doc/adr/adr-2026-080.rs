use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "The Alibaba provider is reached with a short-lived session: a role or an OIDC token in, a session out",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    subsystems: &[taxon!(Subsystem, Alibaba), taxon!(Subsystem, Config)],
    context: r"
        adr-2026-079 settles how a Yandex service-account key becomes a short-lived
        token: the key signs one JWT and is gone. Alibaba Cloud does not work that
        way. Every request is signed (the V3 scheme, ACS3-HMAC-SHA256) with an
        AccessKey pair: HMAC-SHA256 keyed by the AccessKey secret over a hash of
        the canonical request. The secret signs each call, so it cannot be dropped
        after one use; whatever is carried is in memory for as long as it is used.

        What makes the pair safe to carry is how long it lives. A pair issued by
        STS comes with a SecurityToken and an Expiration, and AssumeRoleWithOIDC
        lasts 3600 seconds by default, 900 at the least. A pair without a
        SecurityToken is a long-lived AccessKey.

        Alibaba offers two sources that hold no secret of ours at all, read on
        2026-11-02 from its own documentation. An ECS instance with a RAM role
        attached hands the role's session to code on the instance through the
        metadata service, which has a hardened mode: a token is fetched with a PUT
        to /latest/api/token, and the credentials with a GET to
        /latest/meta-data/ram/security-credentials/<role> carrying that token.
        The answer holds AccessKeyId, AccessKeySecret, SecurityToken, Expiration,
        LastUpdated and Code. And AssumeRoleWithOIDC exchanges an OIDC token for a
        session; the call is anonymous, with no signature and no AccessKey.

        The endpoints follow one pattern, service.region.aliyuncs.com, for ecs,
        vpc and sts, read from each service's endpoint page, and the international
        regions use the same pattern as the rest.
    ",
    decision: r"
        The carried credential is a session: AccessKeyId, AccessKeySecret,
        SecurityToken and Expiration. It is refreshed a minute before it expires.
        A pair without a SecurityToken is a long-lived AccessKey and is refused by
        name wherever one is offered. No Debug output prints a secret or a token.

        Version one has three sources, and no others.

        An instance RAM role, through the metadata service in hardened mode only.
        The token is fetched first and the credentials after it. The mode that
        needs no token is not used, not even when the hardened one fails, because
        a request forged inside the instance can reach it.

        An OIDC token. The fleet names a file that holds the token, the OIDC
        provider and the role; the token is exchanged at STS by AssumeRoleWithOIDC.
        The file is read again at each exchange and never kept, because the
        platform that wrote it renews it. Nothing of Alibaba's is stored.

        A session in the fleet document: access_key_id, access_key_secret and
        security_token, with an expiration where it is known. It is for tests and
        for CI, as iam_token is for Yandex. A fleet that gives the pair without
        the security token is refused by name, and so is a session already past
        its expiration.

        Not in version one: AssumeRole from a long-lived AccessKey, and the
        credentials of the aliyun command line or of the environment.

        Requests are signed with the V3 scheme by ring, which is already linked and
        holds HMAC and SHA-256. The scheme is written out in the crate and held to a
        known answer computed apart from the code, with OpenSSL, over a fixed
        request: Alibaba's documentation gives no worked example with a secret to
        check against. The first recorded exchange with the real API is what
        confirms the scheme in the end.

        Wiping. The session's fields, and every buffer nerpa owns that held them,
        are zeroized when the session is replaced or dropped. The HMAC key ring
        builds from the secret is a copy ring does not wipe; it lives no longer than
        the session, which is the bound this decision accepts.

        The crate carries its own HTTP seam, with the verbs the metadata service and
        the API need, beside the one the Yandex crate carries. Two copies are cheaper
        than a shared crate drawn from one example; at the third cloud they are
        extracted.

        Resource identity, how a VPC or an instance is found again, is not decided
        here: the documentation does not say names are unique.
    ",
    trade_offs: &[
        "The secret signs every call, so it is in memory for the whole session; the window is the session's life, an hour by default, where a Yandex key's was one signature.",
        "Refusing the metadata service's unauthenticated mode costs nothing on an instance that supports the hardened one, and an instance that does not is refused with the reason.",
        "A token file read by group costs a readable bearer for as long as the token lives; the file belongs to the platform that mounts it, which chooses its mode, so no owner-only rule is imposed on it.",
        "ring copies the secret into its HMAC key and does not wipe it; the copy dies with the session.",
    ],
    constraints: &[
        "A pair without a security token is refused by name, held by a test. The executors are w-ali-credentials for the sources and w-ali-fleet-cloud for the fleet document.",
        "The metadata service is called in hardened mode only, and the token is fetched before the credentials, held by a recorded exchange.",
        "AssumeRoleWithOIDC carries no signature and no AccessKey, and the OIDC token is never printed, held by a test.",
        "A session is refreshed before it expires, held by a test.",
        "No Debug output of the provider, the fleet or a session prints a secret or a token, held by a test.",
        "Requests are signed to a known answer computed with OpenSSL, apart from the code, over a fixed request, held by a test; the documentation gives no worked example with a secret.",
    ],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 11, 2),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[crate::rfc::rfc_2026_002],
    introduces_norm: true,
);
