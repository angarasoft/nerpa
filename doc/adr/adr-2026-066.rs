use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "The docs are declared at L2: their work lands only with a gate proof",
    status: DocStatus::Active,
    subsystems: &[taxon!(Subsystem, Docs)],
    context: r"
        Wave 4 brings each subsystem to L2 one at a time, and `Os` was first
        (0058), then `Core` (0059), `Config` (0060), `Proto` (0061), `Executor`
        (0062), `Transport` (0063), `Cli` (0064), then `Gate` (0065). `Docs` is
        next: its invariants — a reference to a record that no longer exists does
        not compile, a superseded decision names its successor and that successor
        exists, and converting a Markdown record to the registry and back loses
        nothing — are enforced by the registry itself, against the pilot
        rfc-2026-001. L2 is the verification layer — a catalog of gates, a
        journal, and a proof at every `Landed` — where «done» stops being an
        opinion and becomes an audit.
    ",
    decision: r"
        The docs are declared at L2. A work item whose radius reaches `Docs` lands
        only with a gate proof: a journal record, the gate that ran, and the proof
        that it passed. The independence a change's radius requires is a review
        record, never an assertion carried by the work item itself.
    ",
    trade_offs: &[
        "Following Gate with Docs costs a ninth slower landing while the proof is learned on the registry, and buys that the record every other proof is written against is itself held to the same audit.",
    ],
    constraints: &[
        "A work item that reaches Docs without a gate proof does not land.",
        "The level may only grow; lowering it is a decision of its own.",
    ],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 29),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[crate::rfc::rfc_2026_001],
);
