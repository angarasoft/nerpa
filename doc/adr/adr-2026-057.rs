use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "An invariant is a typed record whose status says how far it was carried",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    subsystems: &[taxon!(Subsystem, Pilot), taxon!(Subsystem, Docs)],
    context: r"
        docs/invariants.toml holds 62 guarantees and, for the built ones, the
        code that enforces them and the tests that prove it. A TOML table cannot
        refuse an Enforced entry that names no adversarial test, cannot keep an
        enforced_by anchor in the code itself, and cannot make a renamed test a
        compile error. The registry's shape is right; it is the type that is
        missing.
    ",
    decision: r"
        An invariant is a record of its own type, the local invariant!: the
        INV- key, a status as a variant — Planned, Claimed with a described
        bypass, or Enforced, which is either Unrepresentable or a described
        bypass carried out by named adversarial tests. enforced_by and tests are
        anchors on the code and on the test functions; rationale and bypass stay
        prose. Each // ENFORCES: becomes a #[doc_anchor] with an id of its own,
        because anchor ids are unique. The type is local until dacc ships an
        equivalent.
    ",
    trade_offs: &[
        "Enforced cannot be written without its adversarial tests, so a record is refused at compile time rather than reported by an audit.",
        "A fourth local type is a fourth carrier; the cost is accepted because the invariants are the guarantees the product is sold on.",
    ],
    constraints: &[
        "An Enforced invariant is Unrepresentable, or names the adversarial tests that carry out its bypass.",
        "The INV- key is kept, so code and prose go on naming the same guarantee.",
    ],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 28),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[crate::rfc::rfc_2026_001],
    introduces_norm: true,
);
