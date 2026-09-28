use crate::invariant::Status;
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-STATE-001"),
    status: Status::Planned,
    statement: NonEmptyStr::new("State holds references to secrets and digests of their values, never the values."),
    rationale: r"Terraform state carries passwords in clear text, which is why it needs whole-file encryption and still leaks through every backup. A state that never held the value has nothing to leak.",
    enforced_by: &[],
    tests: &[],
);
