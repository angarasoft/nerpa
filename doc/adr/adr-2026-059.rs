use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "The core is declared at L2: its work lands only with a gate proof",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    subsystems: &[taxon!(Subsystem, Core)],
    context: r"
        Wave 4 brings each subsystem to L2 one at a time, and `Os` was first
        (0058). `Core` is next: its four open divergences — a stale attribute
        reference, a lost machine stopping a whole read, a world-readable state
        file, and a control-domain resource disappearing from the plan — are
        closed in code and in tests. L2 is the verification layer — a catalog of
        gates, a journal, and a proof at every `Landed` — where «done» stops
        being an opinion and becomes an audit.
    ",
    decision: r"
        The core is declared at L2. A work item whose radius reaches `Core`
        lands only with a gate proof: a journal record, the gate that ran, and
        the proof that it passed. The independence a change's radius requires is
        a review record, never an assertion carried by the work item itself.
    ",
    trade_offs: &[
        "Following Os with Core costs a second slower landing while the proof is learned on a subsystem with a wider radius, and buys that the engine — the largest subsystem — is held to the same audit as the provider that was proven first.",
    ],
    constraints: &[
        "A work item that reaches Core without a gate proof does not land.",
        "The level may only grow; lowering it is a decision of its own.",
    ],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 29),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[crate::rfc::rfc_2026_002],
);
