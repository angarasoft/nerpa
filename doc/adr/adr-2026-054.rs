use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "A commit carries nerpa's meaning in dacc's form, with a basis",
    status: DocStatus::Active,
    subsystems: &[taxon!(Subsystem, Gate), taxon!(Subsystem, Pilot)],
    context: r"
        nerpa's closed list of commit types exists to keep distinctions that
        dacc's list drops: CHANGE rather than REFACTOR, and PERF and CI. The
        meaning has to survive the move; only the form and the mechanics come from
        dacc.
    ",
    decision: r"
        Convention B: nerpa's types — FEAT, FIX, PERF, CHANGE, DOCS, TEST, CI,
        CHORE, plus ADR and PLAN — in dacc's form, [TYPE](scope): subject, with
        a required scope naming a subsystem of the taxonomy and a basis trailer
        Dacc-Work: wNNNN on every commit. A wrong type is answered with the
        right one, not with the whole list. English throughout, including what the
        tool writes.
    ",
    trade_offs: &[
        "dacc's tool is used for making commits, so one gate holds the rules instead of two.",
        "dacc refuses revert, fixup and merge subjects, and the cost is that git's own tools stop fitting; nerpa has never used them, and dacc learns to accept a revert of an accepted subject before the first one is needed.",
    ],
    constraints: &[
        "A commit after the switch carries a basis; a commit without one is refused.",
        "The subject is English, as everything in the repository is.",
    ],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 28),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[crate::rfc::rfc_2026_001],
);
