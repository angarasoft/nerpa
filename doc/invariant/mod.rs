//! The invariant type: what must never break, how it could, and whether anybody
//! attacked it. Local to nerpa, introduced by `adr-2026-057`, and retired when
//! dacc ships an equivalent.
//!
//! An invariant's status is a variant so that a record cannot say `Enforced`
//! without also saying how a violation was carried out — either the types make
//! it impossible to write (`Unrepresentable`), or a described bypass is named
//! and the adversarial tests that carry it out are listed.

use dacc_core::{AnchorId, NonEmptyStr};

/// How far an invariant has been carried.
#[derive(Debug)]
pub enum Status {
    /// Decided in the architecture, not yet built: no code, no tests, no bypass.
    Planned,
    /// Code and tests exist; the bypass is described but not yet tried.
    Claimed {
        /// The bypass as it would actually look, described in prose.
        bypass: NonEmptyStr,
    },
    /// Code, tests and the bypass's attempt all exist.
    Enforced(Enforced),
}

/// An enforced invariant: the violation is either unrepresentable, or a
/// described bypass is carried out by adversarial tests.
#[derive(Debug)]
pub enum Enforced {
    /// The types make the violation impossible to write, so no test could carry it out.
    Unrepresentable,
    /// A described bypass, carried out by the adversarial tests that assert it is refused.
    Adversarial {
        /// The bypass as it would actually look, described in prose.
        bypass: NonEmptyStr,
        /// The adversarial tests that carry out the bypass and assert it is refused.
        tests: &'static [AnchorId],
    },
}

/// One invariant of the registry.
#[derive(Debug)]
pub struct Invariant {
    /// The `INV-` key, kept so code and prose go on naming the same guarantee.
    pub id: NonEmptyStr,
    /// How far the invariant has been carried.
    pub status: Status,
    /// What must never break, in one sentence.
    pub statement: NonEmptyStr,
    /// Why it must hold, and what a violation would look like.
    pub rationale: &'static str,
    /// Code anchors where the invariant is enforced.
    pub enforced_by: &'static [AnchorId],
    /// Test anchors that prove it.
    pub tests: &'static [AnchorId],
}

/// Registers an invariant. The identifier is the file name, as for knowledge.
#[macro_export]
macro_rules! invariant {
    ($($field:ident : $value:expr),* $(,)?) => {
        /// The invariant of this file.
        pub static INVARIANT: $crate::invariant::Invariant = $crate::invariant::Invariant {
            $($field: $value),*
        };
    };
}
