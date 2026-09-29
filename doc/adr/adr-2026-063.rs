use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "The transport is declared at L2: its work lands only with a gate proof",
    status: DocStatus::Active,
    subsystems: &[taxon!(Subsystem, Transport)],
    context: r"
        Wave 4 brings each subsystem to L2 one at a time, and `Os` was first
        (0058), then `Core` (0059), `Config` (0060), `Proto` (0061), then
        `Executor` (0062). `Transport` is next: its wire invariants — a frame
        from another version is refused, a length prefix is checked before
        allocation, a secret travels sealed, and a write is answered with a
        digest read back — are enforced in code and tests, against the baseline
        rfc-2026-003. L2 is the verification layer — a catalog of gates, a
        journal, and a proof at every `Landed` — where «done» stops being an
        opinion and becomes an audit.
    ",
    decision: r"
        The transport is declared at L2. A work item whose radius reaches
        `Transport` lands only with a gate proof: a journal record, the gate
        that ran, and the proof that it passed. The independence a change's
        radius requires is a review record, never an assertion carried by the
        work item itself.
    ",
    trade_offs: &[
        "Following Executor with Transport costs a sixth slower landing while the proof is learned on the transport, and buys that the path between the engine and the executor is held to the same audit as both ends of it.",
    ],
    constraints: &[
        "A work item that reaches Transport without a gate proof does not land.",
        "The level may only grow; lowering it is a decision of its own.",
    ],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 29),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[crate::rfc::rfc_2026_003],
);
