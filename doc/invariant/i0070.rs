use dacc_knowledge::{Enforced, InvariantStatus};
use dacc_core::NonEmptyStr;

dacc_knowledge::invariant!("i0070",
    specification: crate::rfc::rfc_2026_001,
    status: InvariantStatus::Enforced(Enforced::Unrepresentable),
    statement: NonEmptyStr::new("INV-PILOT-004: A superseded decision names its successor, and that successor exists."),
    rationale: r"The status is a variant, and the variant that says a decision is superseded holds the reference to what supersedes it: naming nothing is not a shape the record can take, and naming a record that does not exist does not compile. The walk beside it checks the residue the type leaves open — that the named successor is a decision of this registry.",
    enforced_by: &[crate::anchor::the_superseded_variant_carries_its_successor],
    tests: &[crate::anchor::a_superseded_decision_names_a_successor_that_exists],
);
