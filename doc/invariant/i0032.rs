use crate::invariant::{Enforced, Status};
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-SECRET-002"),
    status: Status::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("A secret value written where a digest was meant to go, or two different values digesting alike so a rotation is not noticed."), tests: &[crate::anchor::values_that_are_not_equal_do_not_digest_equal, crate::anchor::the_same_value_under_two_keys_digests_differently] }),
    statement: NonEmptyStr::new("A secret value is reduced to a keyed digest before it can be written into state or a plan."),
    rationale: r#"Terraform's state is a plaintext dump of every credential its configuration touched, and whole-file encryption is a bandage over keeping what nobody needed to keep. Two places see a real credential — the planner, comparing what a machine has against what the store holds, and the step that hands a provider what to write — and both reduce it on the way out. `Recorded` has no path back to a value, so the reduction cannot be undone downstream."#,
    enforced_by: &[crate::anchor::inv_secret_002_2, crate::anchor::inv_secret_002_3, crate::anchor::planner_differences],
    tests: &[crate::anchor::a_password_reaches_the_file_and_nothing_else, crate::anchor::a_rotated_secret_is_reported_as_changed, crate::anchor::a_rotated_secret_leaves_no_unkeyed_digest_of_the_old_one_in_the_plan, crate::anchor::the_same_value_under_two_keys_digests_differently, crate::anchor::values_that_are_not_equal_do_not_digest_equal],
);
