use crate::invariant::Status;
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-TRUST-001"),
    status: Status::Planned,
    statement: NonEmptyStr::new("No run proceeds without an actor identity; static keys are not a mode."),
    rationale: r#"Not a recommendation but an absence: there is no code path that reads a long-lived key, so no deadline can produce one."#,
    enforced_by: &[],
    tests: &[],
);
