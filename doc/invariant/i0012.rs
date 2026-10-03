use dacc_knowledge::{Enforced, InvariantStatus};
use dacc_core::NonEmptyStr;

dacc_knowledge::invariant!("i0012",
    specification: crate::rfc::rfc_2026_005,
    status: InvariantStatus::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("A change applied by re-reading the graph rather than from the plan, so what was approved and what happens can differ."), tests: &[crate::anchor::a_file_edited_by_hand_is_brought_back] }),
    statement: NonEmptyStr::new("INV-PLAN-004: A plan carries everything needed to apply it, without the configuration."),
    rationale: r"A plan is what gets signed and what gets handed to something that never saw the program. Carrying only the attributes that differ would make it unusable on its own — an unchanged path is still needed to write the file. Deltas are for reading; the desired state is for doing.",
    enforced_by: &[crate::anchor::inv_plan_004_1, crate::anchor::inv_plan_004_2],
    tests: &[crate::anchor::a_file_edited_by_hand_is_brought_back, crate::anchor::an_unresolved_value_is_settled_from_the_stage_before_it],
);
