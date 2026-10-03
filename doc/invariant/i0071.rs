use dacc_knowledge::{Enforced, InvariantStatus};
use dacc_core::NonEmptyStr;

dacc_knowledge::invariant!("i0071",
    specification: crate::rfc::rfc_2026_001,
    status: InvariantStatus::Enforced(Enforced::Unrepresentable),
    statement: NonEmptyStr::new("INV-PILOT-005: No work item lands with a radius above its slice's ceiling."),
    rationale: r"The build asserts the ceiling over every work item before the registry compiles, so a record above its slice's ceiling is a build error rather than a finding. The same predicate runs beside it over the compiled plan.",
    enforced_by: &[crate::anchor::the_radius_ceiling_is_asserted_over_every_work_item],
    tests: &[crate::anchor::the_radius_ceiling_holds_over_the_plan],
);
