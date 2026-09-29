use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "The config is declared at L2: its work lands only with a gate proof",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    subsystems: &[taxon!(Subsystem, Config)],
    context: r"
        Wave 4 brings each subsystem to L2 one at a time, and `Os` was first
        (0058), then `Core` (0059). `Config` is next: its invariants — that an
        unresolved value never decides which resources exist, that a secret
        reaches a resource and nothing else, that a fact decides an attribute
        and never existence, and that a configuration names the build it needs
        — are enforced in code and tests, against the baseline rfc-2026-004. L2
        is the verification layer — a catalog of gates, a journal, and a proof
        at every `Landed` — where «done» stops being an opinion and becomes an
        audit.
    ",
    decision: r"
        The config is declared at L2. A work item whose radius reaches `Config`
        lands only with a gate proof: a journal record, the gate that ran, and
        the proof that it passed. The independence a change's radius requires is
        a review record, never an assertion carried by the work item itself.
    ",
    trade_offs: &[
        "Following Core with Config costs a third slower landing while the proof is learned on the configuration language, and buys that the language — the part every author writes — is held to the same audit as the engine and the provider.",
    ],
    constraints: &[
        "A work item that reaches Config without a gate proof does not land.",
        "The level may only grow; lowering it is a decision of its own.",
    ],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 29),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[crate::rfc::rfc_2026_004],
);
