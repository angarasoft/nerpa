use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "One kind of knowledge, one typed carrier; the prefix is the type",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    subsystems: &[taxon!(Subsystem, Pilot), taxon!(Subsystem, Docs), taxon!(Subsystem, Gate)],
    context: r"
        nerpa keeps one kind of knowledge in one place, but the carriers are
        Markdown and the checks are a validator that reads text and does not
        resolve a reference to a record. Converting onto dacc needs a target:
        which kinds of document exist, how each is typed, and how a reader tells
        them apart from the name alone.
    ",
    decision: r"
        The catalogue, and the prefix that names each kind in a file name:
        a — decision (adr!), r — specification (rfc!), p — proposal (local
        proposal!), v — review (local review!), i — invariant (local invariant!),
        o — observation (local observation!), t — thrust, s — slice, w — work.
        The four local types are declared here, introduced each by the failure
        that needed it, and retired when dacc ships an equivalent.

        Numbers survive: the Markdown decision 0026 becomes adr-2026-026, and the
        next decision keeps the next number. A number cited in code, commits and
        reviews goes on meaning the same record.
    ",
    trade_offs: &[
        "The prefix is one letter, so it is terse rather than spelled out; the full name is a projection's job, not the file name's.",
        "Local types are a second carrier until dacc catches up, and the cost is keeping two carriers in step; each is fenced by the failure that needed it, so the debt stays visible.",
    ],
    constraints: &[
        "A prefix names exactly one kind; two kinds never share a prefix.",
        "A converted record keeps its number; conversion changes the carrier, never the reasoning.",
    ],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 28),
    breaking: Breaking::No,
    introduces_norm: false,
    code_refs: &[],
    related_rfcs: &[crate::rfc::rfc_2026_001],
);
