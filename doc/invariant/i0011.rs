use dacc_knowledge::{Enforced, InvariantStatus};
use dacc_core::NonEmptyStr;

dacc_knowledge::invariant!("i0011",
    specification: crate::rfc::rfc_2026_002,
    status: InvariantStatus::Enforced(Enforced::Adversarial {
        bypass: NonEmptyStr::new("A declared attribute the provider does not report back after applying — it is never settled, so every run proposes the same update and no run is ever empty. Refused at apply: the provider refuses an attribute it does not own, so the change is refused rather than converging on paper."),
        tests: &[crate::anchor::a_declared_attribute_the_provider_does_not_own_is_refused, crate::anchor::planning_after_applying_produces_an_empty_plan],
    }),
    statement: NonEmptyStr::new("INV-PLAN-003: Applying a plan and planning again produces an empty plan."),
    rationale: r"Convergence is the property this tool exists to have, and the one nobody remembers to assert. It rests on references resolving against what already exists: a value read from another resource is unresolved in the text but known in the world once that resource is there, and without that the reader would differ on every run for ever. A declared attribute the provider does not own would settle never and re-plan for ever, so it is refused instead.",
    enforced_by: &[crate::anchor::planner_resolve],
    tests: &[crate::anchor::planning_after_applying_produces_an_empty_plan, crate::anchor::a_resource_that_already_matches_produces_nothing, crate::anchor::a_reference_to_something_that_exists_is_already_known, crate::anchor::a_declared_attribute_the_provider_does_not_own_is_refused, crate::anchor::a_reference_to_an_attribute_changed_in_the_same_plan_resolves_to_the_new_value],
);
