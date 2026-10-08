use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "The baseline: what nerpa and dacc stood at before the switch",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    subsystems: &[taxon!(Subsystem, Pilot), taxon!(Subsystem, Gate)],
    context: r"
        Every hypothesis in the pilot is measured against a baseline written
        before anything changes. A baseline written after the data is in is a
        description, not a test.
    ",
    decision: r"
        At the switch nerpa holds 49 decision records, 3 RFCs and 62 invariants
        in Markdown, and 8 roadmap stages. The records a change touches — decision,
        invariant, changelog, roadmap, architecture — are measured over the last
        twenty changes, and that measurement is the baseline of H7.

        The count is pinned to the commit it was measured on, so a decision added
        overnight does not corrupt the number: the number and its commit travel
        together.
    ",
    trade_offs: &[
        "Pinning a count to a commit makes it un-live by design, and the cost is re-measuring when the count moves; that is the point, not a defect.",
    ],
    constraints: &[
        "A baseline figure names the commit it was measured on.",
    ],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 28),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[crate::rfc::rfc_2026_001],
    introduces_norm: false,
);
