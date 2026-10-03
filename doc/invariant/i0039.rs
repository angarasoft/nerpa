use dacc_knowledge::{Enforced, InvariantStatus};
use dacc_core::NonEmptyStr;

dacc_knowledge::invariant!("i0039",
    specification: crate::rfc::rfc_2026_002,
    status: InvariantStatus::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("A change applied without the record being called after it — a new branch in the loop, or an early `continue`. Nothing in the types ties one to the other, so the loop is the only thing keeping them in step."), tests: &[crate::anchor::what_was_already_made_is_recorded_before_the_next_thing_is_touched] }),
    statement: NonEmptyStr::new("INV-STATE-005: What a run has already made is written down before it touches the next thing."),
    rationale: r"A run that stops half way has made real changes. State used to be written once, after the whole loop returned, so anything that stopped it — a machine that went away, a contract violation panicking by design, somebody's Ctrl-C — left every resource that run had already made with nothing recording that it exists. They were not rolled back; they were forgotten, and the next run would propose making them again beside the ones already there.

`apply` takes a place to write to and uses it after every change. What that costs is the caller's decision, which is why it is a callback rather than a path: the command writes the state file each time, and a test that only wants to know what ended up in state can do nothing at all. An adoption is the one change not written down after itself, because it made nothing: see INV-STATE-006.",
    enforced_by: &[crate::anchor::inv_state_005_2, crate::anchor::inv_state_005_3, crate::anchor::inv_state_005_4, crate::anchor::run_wrote],
    tests: &[crate::anchor::a_file_is_written_and_the_second_run_has_nothing_to_do, crate::anchor::a_resource_removed_from_the_configuration_is_removed_from_the_machine],
);
