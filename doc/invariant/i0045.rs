use crate::invariant::{Enforced, Status};
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-PLAN-007"),
    status: Status::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("A value whose size is known only after planning: a secret, sealed before the check runs. The provider refuses it before anything is sent, so it fails at apply rather than on the wire, and earlier changes in that run will already have been made."), tests: &[crate::anchor::a_value_too_large_for_one_operation_is_refused_when_planning, crate::anchor::content_too_large_is_refused_before_the_machine_is_asked] }),
    statement: NonEmptyStr::new("A value too large to carry to a machine in one operation is refused when planning, not discovered when applying."),
    rationale: r"A frame has a limit because a length prefix is an instruction to allocate (INV-WIRE-002), and nothing checked the content that has to fit in one. A file over the limit planned cleanly, and its batch failed at apply with an error naming no resource, after earlier changes in the run had already been made. A provider declares the most an attribute may carry; the planner refuses a known value over it, and the provider checks again before sending, for the value planning cannot see.",
    enforced_by: &[crate::anchor::inv_plan_007_2, crate::anchor::planner_differences],
    tests: &[crate::anchor::a_value_too_large_for_one_operation_is_refused_when_planning, crate::anchor::the_os_provider_bounds_content_at_what_one_operation_carries, crate::anchor::content_too_large_is_refused_before_the_machine_is_asked],
);
