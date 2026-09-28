use crate::invariant::Status;
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-PLAN-003"),
    status: Status::Claimed { bypass: NonEmptyStr::new("A declared attribute the provider does not report back after applying. It is never settled, so every run proposes the same update and no run is ever empty. Reproduced: a file declared with an attribute the `os` provider does not own converges on paper and re-plans for ever.") },
    statement: NonEmptyStr::new("Applying a plan and planning again produces an empty plan."),
    rationale: r"Convergence is the property this tool exists to have, and the one nobody remembers to assert. It rests on references resolving against what already exists: a value read from another resource is unresolved in the text but known in the world once that resource is there, and without that the reader would differ on every run for ever.",
    enforced_by: &[crate::anchor::planner_resolve],
    tests: &[crate::anchor::planning_after_applying_produces_an_empty_plan, crate::anchor::a_resource_that_already_matches_produces_nothing, crate::anchor::a_reference_to_something_that_exists_is_already_known],
);
