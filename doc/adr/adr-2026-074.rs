use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "A norm is declared when it is introduced, not backfilled",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    subsystems: &[taxon!(Subsystem, Pilot), taxon!(Subsystem, Gate), taxon!(Subsystem, Docs)],
    context: r"
        dacc 0.6.0 added introduces_norm to decisions and specifications: a
        record that declares a norm — a binding promise that requires an
        executor — says so, and the compiler's norms_reconciled refuses a norm
        without an executor. The move onto 0.6.0 set the field to false on all
        72 records mechanically, because the classification is a semantic act,
        not a mechanical one, and the migration's job was to ride the release,
        not to re-judge every record.

        The main line classified sixteen records at its own move onto 0.6.0,
        before this decision rejoined it; those marks stand, and this rule
        governs the rest.
    ",
    decision: r"
        introduces_norm is marked when a norm is introduced, not backfilled. A
        decision or specification sets introduces_norm: true only when it
        declares a binding promise that requires an executor — a landed work
        with origin Decision or Specification, or an obligation — and it names
        that executor in the same change. The records migrated onto 0.6.0 stay
        false until one of them is found to declare a norm; the classification
        is made as each record is touched, not in one pass over the registry.
    ",
    trade_offs: &[r"
        Classifying on demand keeps the migration mechanical and leaves no
        false norm. The cost is that, until a record is touched, a norm it
        declared is not marked — a reader cannot tell 'not a norm' from 'not
        yet judged', which is why the marker is not read as proof of absence.
    "],
    constraints: &[
        "A record marked introduces_norm: true carries its executor in the same change, or the build refuses it once the norm check is emitted.",
        "The records migrated onto 0.6.0 are not re-classified in one pass; each is judged when it is next touched.",
    ],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 10, 12),
    breaking: Breaking::No,
    introduces_norm: false,
    code_refs: &[],
    related_rfcs: &[crate::rfc::rfc_2026_001],
);
