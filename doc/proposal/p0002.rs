use crate::proposal::ProposalStatus;
use dacc_core::{NonEmptyStr, nonempty_str};

crate::proposal!(
    title: NonEmptyStr::new("Replacing an installed executor through itself"),
    status: ProposalStatus::Draft,
    body: include_str!("../../docs/rfcs/0002-replacing-an-installed-executor.md"),
    decided: &[],
    specification: crate::rfc::rfc_2026_002,
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 11),
);
