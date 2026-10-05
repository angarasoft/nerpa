use crate::proposal::ProposalStatus;
use dacc_core::{NonEmptyStr, nonempty_str};

crate::proposal!(
    title: NonEmptyStr::new("Control-domain apply: a provider that acts without a machine"),
    status: ProposalStatus::Draft,
    body: include_str!("../../docs/rfcs/0004-control-domain-apply.md"),
    decided: &[],
    specification: crate::rfc::rfc_2026_002,
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 10, 5),
);
