use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "An independent reading is a record of its own, not a note in somebody else's",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    subsystems: &[taxon!(Subsystem, Pilot)],
    context: r"
        The verdict on the pilot is read first by a model other than the one
        that did most of the work, and that reading has to stand on its own: its
        angle, its findings, and what became of each. A note inside somebody
        else's record has no place to say those, and no way to be written by a
        reader who writes no other record.
    ",
    decision: r"
        An independent reading is a record of its own type, the local review!:
        its angle, its findings, and what became of each, filed at doc/review/.
        The type is local until dacc ships an equivalent, at which point a
        decision supersedes it and a work item converts its records.
    ",
    trade_offs: &[
        "A second local type is a second carrier to keep; the cost is accepted because the verdict's independence has no other carrier.",
    ],
    constraints: &[
        "A review's author writes no other pilot record.",
    ],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 28),
    breaking: Breaking::No,
    introduces_norm: false,
    code_refs: &[],
    related_rfcs: &[crate::rfc::rfc_2026_001],
);
