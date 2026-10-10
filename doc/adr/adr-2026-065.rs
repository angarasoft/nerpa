use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "The gate is declared at L2: its work lands only with a gate proof",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    subsystems: &[taxon!(Subsystem, Gate)],
    context: r"
        Wave 4 brings each subsystem to L2 one at a time, and `Os` was first
        (0058), then `Core` (0059), `Config` (0060), `Proto` (0061), `Executor`
        (0062), `Transport` (0063), then `Cli` (0064). `Gate` is next: its
        invariants — a work item cannot be landed without a gate proof recorded
        on the tree that landed it, every commit carries a basis naming a work
        item in its own tree, and no work item lands with a radius above its
        slice's ceiling — are enforced by the gate itself, against the pilot
        rfc-2026-001. L2 is the verification layer — a catalog of gates, a
        journal, and a proof at every `Landed` — where «done» stops being an
        opinion and becomes an audit.
    ",
    decision: r"
        The gate is declared at L2. A work item whose radius reaches `Gate` lands
        only with a gate proof: a journal record, the gate that ran, and the proof
        that it passed. The independence a change's radius requires is a review
        record, never an assertion carried by the work item itself.
    ",
    trade_offs: &[
        "Following Cli with Gate costs an eighth slower landing while the proof is learned on the gate that checks every other proof, and buys that the place where every other L2 claim is verified is itself held to the same audit.",
    ],
    constraints: &[
        "A work item that reaches Gate without a gate proof does not land.",
        "The level may only grow; lowering it is a decision of its own.",
    ],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 29),
    breaking: Breaking::No,
    introduces_norm: false,
    code_refs: &[],
    related_rfcs: &[crate::rfc::rfc_2026_001],
);
