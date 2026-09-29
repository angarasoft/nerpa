use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "A proposal is a change worked out, measured and attacked before its decisions",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    subsystems: &[taxon!(Subsystem, Pilot), taxon!(Subsystem, Docs)],
    context: r"
        The three RFCs in docs/rfcs/ are more than their decided parts: each holds
        a body of design, measurements with provenance, prior art, alternatives,
        and a review log. A decision record cannot hold that prose, and a
        specification holds only the contract parts. The RFC as a carrier has no
        type, so nothing checks that a proposal names what it decided or that its
        review rounds ended in work items, decisions or stated rejections.
    ",
    decision: r"
        A proposal is a record of its own type, the local proposal!: its title,
        status as a variant (Draft, InReview, Accepted, Withdrawn), a Markdown
        body included by path, the decisions it settled, and the specification
        holding its contract parts. The body lives in docs/rfcs/ and is included
        with include_str!, so deleting it breaks the build. The type is local
        until dacc ships an equivalent.
    ",
    trade_offs: &[
        "A third local type is a third carrier to keep; the cost is accepted because no other record holds a change's reasoning before its decisions.",
    ],
    constraints: &[
        "A proposal's body is the Markdown it was written in, byte for byte.",
        "Every review round of a proposal ends as work items, decisions, or a stated rejection.",
    ],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 28),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[crate::rfc::rfc_2026_001],
);
