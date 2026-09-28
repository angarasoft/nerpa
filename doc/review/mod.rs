//! The review type: an independent reading — its angle, its findings, and what
//! became of each. Local to nerpa, introduced by `adr-2026-055`, and retired
//! when dacc ships an equivalent.

use chrono::NaiveDate;
use dacc_core::{NonEmpty, NonEmptyStr};

/// One independent reading of the pilot's work.
#[derive(Debug)]
pub struct Review {
    /// What was read, in one line.
    pub title: NonEmptyStr,
    /// The angle the reading was taken from, so its limits are on the record.
    pub angle: &'static str,
    /// Each finding and what became of it, in one non-empty string.
    pub findings: NonEmpty<NonEmptyStr>,
    /// Who did the reading; for the verdict, a reader who writes no other record.
    pub authors: NonEmpty<NonEmptyStr>,
    /// When the reading happened.
    pub decided_at: NaiveDate,
}

/// Registers a review. The identifier is the file name, as for knowledge.
#[macro_export]
macro_rules! review {
    ($($field:ident : $value:expr),* $(,)?) => {
        /// The review of this file.
        pub static REVIEW: $crate::review::Review = $crate::review::Review {
            $($field: $value),*
        };
    };
}

/// The first review: the conversion, read after it happened.
#[path = "review-2026-001.rs"]
pub mod review_2026_001;

/// The second review: wave 2, measured after it happened.
#[path = "review-2026-002.rs"]
pub mod review_2026_002;

/// Every review, in order. Kept by hand until the scan learns the local type.
pub static ALL_REVIEWS: &[&Review] = &[&review_2026_001::REVIEW, &review_2026_002::REVIEW];
