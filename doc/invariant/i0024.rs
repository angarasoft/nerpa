use dacc_knowledge::{Enforced, InvariantStatus};
use dacc_core::NonEmptyStr;

dacc_knowledge::invariant!("i0024",
    specification: crate::rfc::rfc_2026_003,
    status: InvariantStatus::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("A provider asked once per resource rather than once per machine, which no transport underneath can make fast again."), tests: &[crate::anchor::every_resource_on_one_machine_is_asked_for_at_once, crate::anchor::separate_machines_are_separate_journeys] }),
    statement: NonEmptyStr::new("INV-BATCH-001: Reading a machine takes one journey, however many resources are on it."),
    rationale: r"The difference between reading a fleet in seconds and reading it in minutes, and a property of the interface rather than of any transport: an interface that can only ask about one resource makes the fast version unrepresentable however good the connection underneath is. This is what Ansible gets wrong, one round trip per task. The engine groups by machine before asking, and a batch never spans two.",
    enforced_by: &[crate::anchor::inv_batch_001_1, crate::anchor::run_observe],
    tests: &[crate::anchor::every_resource_on_one_machine_is_asked_for_at_once, crate::anchor::separate_machines_are_separate_journeys, crate::anchor::something_only_state_remembers_travels_with_the_rest],
);
