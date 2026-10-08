use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::DocStatus;

dacc_knowledge::rfc!(
    title: "The wire: what the engine and an installed executor say to each other",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    target: &[
        taxon!(Subsystem, Proto),
        taxon!(Subsystem, Executor),
        taxon!(Subsystem, Transport),
    ],
    goal: r"
        An executor installed anywhere keeps talking to every later engine, and a
        confused or hostile peer cannot turn a length into memory exhaustion or a
        secret into a plaintext reading. Checkable: a frame of another version is
        refused as a version, a frame over the bound is refused before allocation,
        and a secret premise travels sealed and is refused when it arrives bare.

        Only the wire is a compatibility surface that RFC 0001 changes; the
        invariants it introduces live in the invariant registry, and the rest of
        the applying mechanism — shards, the recorder, parallelism — is internal
        design, not contract.

        Corrected: 2026-10-02 — the invariants below were this record's own
        words for four guarantees the registry holds in its own, so the contract
        had no executors the registry could see. They are quoted from the
        registry now — the home the paragraph above names — and one of them,
        the write answered with a digest read back, gained the record it never
        had. The guarantees are the four they always were.
    ",
    input_contract: r"
        The engine's request and the executor's response, framed as a length prefix
        then a CBOR body, over a transport that carries bytes and knows nothing of
        their meaning (0010).
    ",
    output_contract: r"
        Protocol 5: every frame is stamped with its version, read first and refused
        when not understood; frames are bounded and a prefix over the bound is
        refused before a byte is reserved; a file write is answered with a digest
        read back, sealed for the machine when the content is secret, and the engine
        checks it (0041).
    ",
    invariants: nonempty_str![
        "A frame written by a protocol version this build does not know is refused, not partly believed.",
        "A frame's announced size is checked before any of it is allocated.",
        "A premise leaves the planner only sealed for the machine it describes, a write that carries a secret says so, a private process answers with a seal, and an executor refuses any of them that arrives without one.",
        "A write is answered with a digest read back, and the engine checks it.",
    ],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 17),
    decided_by: &[
        crate::adr::adr_2026_0010,
        crate::adr::adr_2026_0040,
        crate::adr::adr_2026_0041,
    ],
    introduces_norm: false,
);
