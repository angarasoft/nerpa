use crate::invariant::Status;
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-TRUST-001"),
    status: Status::Planned,
    statement: NonEmptyStr::new("No run proceeds without an actor identity; static keys are not a mode."),
    rationale: r"Two halves, one already true. The static-key half is an absence: the transport is `ssh`'s own configuration, the `Key` in core is a digest key, and no code path reads a long-lived key, so no deadline can produce one. The actor-identity half is Stage 4 (decision 0009): a run must carry who acts — a person over OIDC or Teleport, CI over workload identity, an agent over a grant — and refuse without one.",
    enforced_by: &[],
    tests: &[],
);
