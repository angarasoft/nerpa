use crate::invariant::{Enforced, Status};
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-REACT-001"),
    status: Status::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("Counting every change the plan holds as a cause whatever its kind: the planner adding an address to what is changing before asking whether anything is sent for it."), tests: &[crate::anchor::a_file_adopted_or_reordered_rouses_nothing] }),
    statement: NonEmptyStr::new("Only a change that sends something to a machine rouses what reacts to it."),
    rationale: r#"A reaction is usually a restart, and the reason one is in the plan is that bouncing a service on every machine in a fleet should not be found out by running it. An adoption or a reorder writes state and leaves the resource as it was, so nothing anything reacts to has moved. Counted as changing, an adopted configuration file restarted its service on every machine a first run adopted. See decision 0026."#,
    enforced_by: &[crate::anchor::planner_walk],
    tests: &[crate::anchor::a_file_adopted_or_reordered_rouses_nothing, crate::anchor::a_service_whose_file_changes_is_roused_and_the_plan_says_by_what],
);
