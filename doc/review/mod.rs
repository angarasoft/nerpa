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
#[path = "v0001.rs"]
pub mod v0001;

/// The second review: wave 2, measured after it happened.
#[path = "v0002.rs"]
pub mod v0002;

/// RFC 0001's first review round.
#[path = "v0003.rs"]
pub mod v0003;

/// RFC 0001's second review round.
#[path = "v0004.rs"]
pub mod v0004;

/// RFC 0001's third review round.
#[path = "v0005.rs"]
pub mod v0005;

/// RFC 0001's narrow review.
#[path = "v0006.rs"]
pub mod v0006;

/// RFC 0001's fifth review round.
#[path = "v0007.rs"]
pub mod v0007;

/// RFC 0001's sixth review round.
#[path = "v0008.rs"]
pub mod v0008;

/// RFC 0001's narrow round.
#[path = "v0009.rs"]
pub mod v0009;

/// Wave 3, read after it happened.
#[path = "v0010.rs"]
pub mod v0010;

/// Wave 4 and the L2 completion, measured after it happened.
#[path = "v0011.rs"]
pub mod v0011;

/// The verdict, read independently against the criteria written before the start.
#[path = "v0012.rs"]
pub mod v0012;

/// Every review, in order. Kept by hand until the scan learns the local type.
pub static ALL_REVIEWS: &[&Review] = &[
    &v0001::REVIEW,
    &v0002::REVIEW,
    &v0003::REVIEW,
    &v0004::REVIEW,
    &v0005::REVIEW,
    &v0006::REVIEW,
    &v0007::REVIEW,
    &v0008::REVIEW,
    &v0009::REVIEW,
    &v0010::REVIEW,
    &v0011::REVIEW,
    &v0012::REVIEW,
];
