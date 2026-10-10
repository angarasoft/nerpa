use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "The pilot is judged against criteria written before it started, by the owner after an independent reading",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    subsystems: &[taxon!(Subsystem, Pilot)],
    context: r"
        The pilot exists to test dacc, not merely to move nerpa onto it. A
        criterion written after the data is in is a description, not a test, so
        every hypothesis names what measures it, where the number comes from, and
        which result stops or narrows the pilot.
    ",
    decision: r"
        Nine hypotheses, each with a measure and a stop threshold, are recorded
        before the pilot starts. H1 compiled references catch disagreements the
        Markdown checks miss; H2 writing a record costs little enough; H3 nobody
        works around the gate; H4 origins stay meaningful; H5 done is proven; H6
        radius is labelled honestly; H7 book-keeping does not grow; H8 conversion
        loses nothing; H9 the local types earn their place.

        The verdict is given by Anishchuk Sergei when Core reaches L2 or eight
        weeks after the switch, whichever comes first. It is read first by a model
        other than the one that did most of the pilot's work; that reading is a
        review record, and its author writes no other pilot record. The verdict is
        one of three: continue, narrow, or stop — the rollback recorded with the
        pilot's risks.
    ",
    trade_offs: &[
        "Writing criteria before the start costs the work of phrasing them, and buys a test that cannot be moved afterwards to fit the data.",
        "The verdict waits for Core at L2 or eight weeks, so a pilot that is merely slow ends on the calendar rather than on a judgement.",
    ],
    constraints: &[
        "A hypothesis without a measure or a stop threshold is not a hypothesis.",
        "The verdict is read first by a different model than the one that did most of the work.",
    ],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 28),
    breaking: Breaking::No,
    introduces_norm: false,
    code_refs: &[],
    related_rfcs: &[crate::rfc::rfc_2026_001],
);
