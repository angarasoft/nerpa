use crate::invariant::{Enforced, Status};
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-REACT-004"),
    status: Status::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("Deciding a restart from the kind of change alone, in the engine or in a provider. A roused Update then applies cleanly, clears its debt and restarts nothing."), tests: &[crate::anchor::a_service_owed_a_restart_is_restarted_when_it_changes_as_well] }),
    statement: NonEmptyStr::new("A change that was roused is acted on as roused, whatever else changed."),
    rationale: r"A service whose own attributes change in the same plan as the file it reacts to is one Update, not an update and a restart, so the provider has to be told it was roused. It was not: the os provider restarted only a React, an Update settled `enable` and `start`, `start` does nothing to a running unit, and the fresh record cleared the debt. A new file, an old process, and nothing owed: the state 0026 exists to make unreachable.",
    enforced_by: &[crate::anchor::inv_react_004_2, crate::anchor::run_apply],
    tests: &[crate::anchor::a_service_owed_a_restart_is_restarted_when_it_changes_as_well, crate::anchor::a_service_changing_and_roused_is_settled_then_restarted],
);
