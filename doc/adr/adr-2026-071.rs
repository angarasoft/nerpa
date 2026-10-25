use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "Recorded exchanges make the Yandex tests honest without a live cloud",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    subsystems: &[taxon!(Subsystem, Yandex)],
    context: r"
        The provider's correctness is its mapping of nerpa changes to Yandex
        calls and back: the requests it builds and the answers it reads. The
        converge tests prove the round trip against a fake cloud written by
        hand, which settles the engine's contract but not the mapping — a
        provider that builds a wrong request against the real API shape is one
        the fake would have to be wrong in exactly the same way to catch. A
        live cloud settles it, and is slow, non-deterministic, and costs money
        every run.

        ADR-070 draws the secret line: a service-account key enters the flow,
        a short-lived token leaves it, and no record carries key or token
        material. Whatever a recording keeps must therefore keep neither.
    ",
    decision: r"
        Real exchanges are recorded once and replayed in tests. A recording
        is one JSON file per scenario: the folder it was taken in, and the
        exchanges — each one the full gRPC path of the call and the protobuf
        request and response bodies as base64. Bodies only: authorization
        metadata is not stored, and the IAM exchange is not recorded at all —
        its answer is a token, which ADR-070 forbids writing down. A replay
        answers IAM locally, verifying the JWT signature as the fake cloud
        does.

        The recording is taken through a tunnel: a server that speaks the
        seven services and forwards every call to the live API, writing the
        exchange as it passes. The tool that runs it is an ignored test,
        driven by environment variables — endpoint, folder, and a credential
        file — and nothing of those enters the tree.

        A replay server answers from the recording, matching each request by
        its full gRPC path and its protobuf body bytes, and refuses a request
        no recording covers. Playback by order would prove less: matching by
        bytes proves the provider sends exactly the request that was recorded,
        which is the mapping the work exists to hold honest.

        The first recording against the live cloud settled what the client's
        TLS posture is. The cloud's certificate chain ends at a root the
        Mozilla bundle does not carry, so the client trusts the operating
        system's bundle — the same store curl and yc consult — and the Mozilla
        bundle leaves the tree; ring stays the one crypto provider, and the
        aws-lc-rs provider is not brought in. A key file written by
        `yc iam key create` carries a marker line before the armor, and the
        client reads it; an https endpoint without a TLS configuration is
        refused at connect, not silently dialled plain. The cloud serves
        compute, vpc and operation on separate hosts, so the tunnel fans each
        call out to the host that serves it — the same mapping a live client
        itself needs.
    ",
    trade_offs: &[
        "Matching by bytes is stricter than playback by order — a replay refuses what a same-order playback would silently answer — and the cost is that any change to a request the provider builds invalidates the recording, which is re-taken rather than edited.",
        "A recording pins the world at the moment it was taken — ids, zones, folder — which is what makes the test deterministic, and is also what makes it blind to anything the cloud changes afterwards; re-taking is the answer, one ignored test run away.",
        "The IAM exchange stays untested against the live cloud by the replay; its contract is held separately by the signature tests against the public key, and the replay's IAM is honest about exactly that part.",
    ],
    constraints: &[
        "A recording stores request and response bodies and nothing else: no authorization metadata, no token, no key (ADR-070).",
        "A replay refuses a request no recording covers; it never invents an answer.",
        "Recordings are re-taken by the recording tool, not edited by hand.",
    ],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 10, 25),
    breaking: Breaking::No,
    introduces_norm: false,
    code_refs: &[],
    related_rfcs: &[crate::rfc::rfc_2026_002],
);
