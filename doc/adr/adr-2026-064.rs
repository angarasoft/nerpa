use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "The cli is declared at L2: its work lands only with a gate proof",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    subsystems: &[taxon!(Subsystem, Cli)],
    context: r"
        Wave 4 brings each subsystem to L2 one at a time, and `Os` was first
        (0058), then `Core` (0059), `Config` (0060), `Proto` (0061), `Executor`
        (0062), then `Transport` (0063). `Cli` is next: its invariants — that the
        plan a person reads and the plan a machine reads are the same object, and
        that an attribute a resource was born with is reported and never turned
        into work — are enforced in code and tests, against the baseline
        rfc-2026-005. L2 is the verification layer — a catalog of gates, a
        journal, and a proof at every `Landed` — where «done» stops being an
        opinion and becomes an audit.
    ",
    decision: r"
        The cli is declared at L2. A work item whose radius reaches `Cli` lands
        only with a gate proof: a journal record, the gate that ran, and the proof
        that it passed. The independence a change's radius requires is a review
        record, never an assertion carried by the work item itself.
    ",
    trade_offs: &[
        "Following Transport with Cli costs a seventh slower landing while the proof is learned on the command line, and buys that the surface a person and an agent both touch is held to the same audit as the engine behind it.",
    ],
    constraints: &[
        "A work item that reaches Cli without a gate proof does not land.",
        "The level may only grow; lowering it is a decision of its own.",
    ],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 29),
    breaking: Breaking::No,
    introduces_norm: false,
    code_refs: &[],
    related_rfcs: &[crate::rfc::rfc_2026_005],
);
