use crate::proposal::ProposalStatus;
use dacc_core::{NonEmptyStr, nonempty_str};

crate::proposal!(
    title: NonEmptyStr::new("Streamed batches"),
    status: ProposalStatus::Draft,
    body: include_str!("../../docs/rfcs/0003-streamed-batches.md"),
    decided: &[],
    specification: crate::rfc::rfc_2026_002,
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 11),
);
