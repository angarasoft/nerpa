//! The proposal type: a change worked out, measured and attacked before its
//! decisions are taken. Local to nerpa, introduced by `adr-2026-056`, and
//! retired when dacc ships an equivalent.

use chrono::NaiveDate;
use dacc_core::{AdrRef, NonEmpty, NonEmptyStr, RfcRef};

/// Where a proposal stands.
#[derive(Debug)]
pub enum ProposalStatus {
    /// Being worked out, not yet put up for review.
    Draft,
    /// Under review; the findings are in the body's review log.
    InReview,
    /// Accepted; its decided parts are decision records.
    Accepted,
    /// Abandoned; the reasoning is kept but the change is not taken.
    Withdrawn,
}

/// One proposal of the registry.
#[derive(Debug)]
pub struct Proposal {
    /// The change, in one line.
    pub title: NonEmptyStr,
    /// Where the proposal stands.
    pub status: ProposalStatus,
    /// The Markdown body, included so deleting it breaks the build.
    pub body: &'static str,
    /// The decisions the proposal settled.
    pub decided: &'static [AdrRef],
    /// The specification holding the contract parts.
    pub specification: RfcRef,
    /// Who worked it out.
    pub authors: NonEmpty<NonEmptyStr>,
    /// When its current status was reached.
    pub decided_at: NaiveDate,
}

/// Registers a proposal. The identifier is the file name, as for knowledge.
#[macro_export]
macro_rules! proposal {
    ($($field:ident : $value:expr),* $(,)?) => {
        /// The proposal of this file.
        pub static PROPOSAL: $crate::proposal::Proposal = $crate::proposal::Proposal {
            $($field: $value),*
        };
    };
}
