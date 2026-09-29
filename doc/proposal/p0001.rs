use crate::proposal::ProposalStatus;
use dacc_core::{NonEmptyStr, nonempty_str};

crate::proposal!(
    title: NonEmptyStr::new("Applying machines in parallel, with state in shards"),
    status: ProposalStatus::Accepted,
    body: include_str!("../../docs/rfcs/0001-applying-in-streamed-batches.md"),
    decided: &[
        crate::adr::adr_2026_0006,
        crate::adr::adr_2026_0015,
        crate::adr::adr_2026_0026,
        crate::adr::adr_2026_0027,
        crate::adr::adr_2026_0031,
        crate::adr::adr_2026_0033,
        crate::adr::adr_2026_0034,
        crate::adr::adr_2026_0040,
        crate::adr::adr_2026_0041,
        crate::adr::adr_2026_0042,
        crate::adr::adr_2026_0043,
    ],
    specification: crate::rfc::rfc_2026_003,
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 17),
);
