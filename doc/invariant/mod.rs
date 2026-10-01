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

/// An invariant record.
pub mod i0001;
/// An invariant record.
pub mod i0002;
/// An invariant record.
pub mod i0003;
/// An invariant record.
pub mod i0004;
/// An invariant record.
pub mod i0005;
/// An invariant record.
pub mod i0006;
/// An invariant record.
pub mod i0007;
/// An invariant record.
pub mod i0008;
/// An invariant record.
pub mod i0009;
/// An invariant record.
pub mod i0010;
/// An invariant record.
pub mod i0011;
/// An invariant record.
pub mod i0012;
/// An invariant record.
pub mod i0013;
/// An invariant record.
pub mod i0014;
/// An invariant record.
pub mod i0015;
/// An invariant record.
pub mod i0016;
/// An invariant record.
pub mod i0017;
/// An invariant record.
pub mod i0018;
/// An invariant record.
pub mod i0019;
/// An invariant record.
pub mod i0020;
/// An invariant record.
pub mod i0021;
/// An invariant record.
pub mod i0022;
/// An invariant record.
pub mod i0023;
/// An invariant record.
pub mod i0024;
/// An invariant record.
pub mod i0025;
/// An invariant record.
pub mod i0026;
/// An invariant record.
pub mod i0027;
/// An invariant record.
pub mod i0028;
/// An invariant record.
pub mod i0029;
/// An invariant record.
pub mod i0030;
/// An invariant record.
pub mod i0031;
/// An invariant record.
pub mod i0032;
/// An invariant record.
pub mod i0033;
/// An invariant record.
pub mod i0034;
/// An invariant record.
pub mod i0035;
/// An invariant record.
pub mod i0036;
/// An invariant record.
pub mod i0037;
/// An invariant record.
pub mod i0038;
/// An invariant record.
pub mod i0039;
/// An invariant record.
pub mod i0040;
/// An invariant record.
pub mod i0041;
/// An invariant record.
pub mod i0042;
/// An invariant record.
pub mod i0043;
/// An invariant record.
pub mod i0044;
/// An invariant record.
pub mod i0045;
/// An invariant record.
pub mod i0046;
/// An invariant record.
pub mod i0047;
/// An invariant record.
pub mod i0048;
/// An invariant record.
pub mod i0049;
/// An invariant record.
pub mod i0050;
/// An invariant record.
pub mod i0051;
/// An invariant record.
pub mod i0052;
/// An invariant record.
pub mod i0053;
/// An invariant record.
pub mod i0054;
/// An invariant record.
pub mod i0055;
/// An invariant record.
pub mod i0056;
/// An invariant record.
pub mod i0057;
/// An invariant record.
pub mod i0058;
/// An invariant record.
pub mod i0059;
/// An invariant record.
pub mod i0060;
/// An invariant record.
pub mod i0061;
/// An invariant record.
pub mod i0062;
/// An invariant record.
pub mod i0063;

/// Every invariant, in order. Kept by hand until the scan learns the local type.
pub static ALL_INVARIANTS: &[&Invariant] = &[
    &i0001::INVARIANT,
    &i0002::INVARIANT,
    &i0003::INVARIANT,
    &i0004::INVARIANT,
    &i0005::INVARIANT,
    &i0006::INVARIANT,
    &i0007::INVARIANT,
    &i0008::INVARIANT,
    &i0009::INVARIANT,
    &i0010::INVARIANT,
    &i0011::INVARIANT,
    &i0012::INVARIANT,
    &i0013::INVARIANT,
    &i0014::INVARIANT,
    &i0015::INVARIANT,
    &i0016::INVARIANT,
    &i0017::INVARIANT,
    &i0018::INVARIANT,
    &i0019::INVARIANT,
    &i0020::INVARIANT,
    &i0021::INVARIANT,
    &i0022::INVARIANT,
    &i0023::INVARIANT,
    &i0024::INVARIANT,
    &i0025::INVARIANT,
    &i0026::INVARIANT,
    &i0027::INVARIANT,
    &i0028::INVARIANT,
    &i0029::INVARIANT,
    &i0030::INVARIANT,
    &i0031::INVARIANT,
    &i0032::INVARIANT,
    &i0033::INVARIANT,
    &i0034::INVARIANT,
    &i0035::INVARIANT,
    &i0036::INVARIANT,
    &i0037::INVARIANT,
    &i0038::INVARIANT,
    &i0039::INVARIANT,
    &i0040::INVARIANT,
    &i0041::INVARIANT,
    &i0042::INVARIANT,
    &i0043::INVARIANT,
    &i0044::INVARIANT,
    &i0045::INVARIANT,
    &i0046::INVARIANT,
    &i0047::INVARIANT,
    &i0048::INVARIANT,
    &i0049::INVARIANT,
    &i0050::INVARIANT,
    &i0051::INVARIANT,
    &i0052::INVARIANT,
    &i0053::INVARIANT,
    &i0054::INVARIANT,
    &i0055::INVARIANT,
    &i0056::INVARIANT,
    &i0057::INVARIANT,
    &i0058::INVARIANT,
    &i0059::INVARIANT,
    &i0060::INVARIANT,
    &i0061::INVARIANT,
    &i0062::INVARIANT,
    &i0063::INVARIANT,
];
