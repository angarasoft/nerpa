use dacc_knowledge::{Enforced, InvariantStatus};
use dacc_core::NonEmptyStr;

dacc_knowledge::invariant!("i0010",
    specification: crate::rfc::rfc_2026_002,
    status: InvariantStatus::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("A graph that cannot be ordered yielding a partial plan rather than none at all."), tests: &[crate::anchor::a_graph_that_cannot_be_ordered_is_not_planned_at_all] }),
    statement: NonEmptyStr::new("INV-PLAN-002: Planning is one pass; stages belong to execution."),
    rationale: r"Two planning passes would mean a plan that is only partly known, which cannot be signed as a whole. Stages order the work; they do not order the thinking. The planner walks the graph once and never returns to a stage it has left.",
    enforced_by: &[crate::anchor::inv_plan_002],
    tests: &[crate::anchor::stages_with_nothing_to_do_are_left_out_but_order_is_kept, crate::anchor::a_graph_that_cannot_be_ordered_is_not_planned_at_all],
);
