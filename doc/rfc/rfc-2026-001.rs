use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::DocStatus;

dacc_knowledge::rfc!(
    title: "The pilot: dacc's promises under test, stated as invariants",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    target: &[taxon!(Subsystem, Pilot), taxon!(Subsystem, Gate), taxon!(Subsystem, Docs)],
    goal: r"
        A record, a plan and a proof are linked by references the compiler checks,
        and the cost of writing and proving them is low enough that nobody works
        around the gate. Checkable: deleting a record breaks every reference to it
        at compile time; a work item cannot be landed without a gate proof on the
        tree that landed it.
    ",
    input_contract: r"
        The registry at doc/, written as code: adr/, rfc/, thrust/, slice/, work/,
        journal/ and taxonomy.rs. A commit after the switch carries a Dacc-Work
        trailer naming a work item in the commit's own tree.
    ",
    output_contract: r"
        Reference constants and lists generated at build time; the journal folded
        into WORK_STATES and CLOSED_SLICES; a build error when a reference does not
        resolve or a machine rule is violated.
    ",
    invariants: nonempty_str![
        "A reference to a record that no longer exists does not compile.",
        "A work item cannot be landed without a gate proof recorded on the same tree.",
        "Every commit after the switch carries a basis naming a work item in the commit's own tree.",
        "A superseded decision names its successor, and that successor exists.",
        "No work item lands with a radius above its slice's ceiling.",
        "Converting a Markdown record to the registry and back loses nothing: the sections compare byte for byte.",
    ],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 28),
    decided_by: &[crate::adr::adr_2026_050],
);
