use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "The os provider is declared at L2: its work lands only with a gate proof",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    subsystems: &[taxon!(Subsystem, Os)],
    context: r"
        Wave 4 brings each subsystem to L2 one at a time, and `Os` is first: its
        three open divergences (w0009, w0010, w0011) are closed in code and in
        tests, and their radius is local, so the loop from a fix to a proof is
        short. L2 is the verification layer — a catalog of gates, a journal, and
        a proof at every `Landed` — where «done» stops being an opinion and
        becomes an audit.
    ",
    decision: r"
        The os provider is declared at L2. A work item whose radius reaches `Os`
        lands only with a gate proof: a journal record, the gate that ran, and
        the proof that it passed. The independence a change's radius requires is
        a review record, never an assertion carried by the work item itself.
    ",
    trade_offs: &[
        "Starting with Os costs a slower first landing while the proof is learned on it, and buys that every later subsystem inherits a proven loop instead of each inventing one.",
    ],
    constraints: &[
        "A work item that reaches Os without a gate proof does not land.",
        "The level may only grow; lowering it is a decision of its own.",
    ],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 29),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[crate::rfc::rfc_2026_002],
);
