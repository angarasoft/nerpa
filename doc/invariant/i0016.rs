use dacc_knowledge::{Enforced, InvariantStatus};
use dacc_core::NonEmptyStr;

dacc_knowledge::invariant!("i0016",
    specification: crate::rfc::rfc_2026_002,
    status: InvariantStatus::Enforced(Enforced::Adversarial {
        bypass: NonEmptyStr::new("A second way of writing state that reaches for `fs::write` instead of the write-beside, sync and rename in `store` — the short form is one line and the durable form is five, and nothing in the types marks which is which. Below that, a disk that acknowledges a sync it has not performed, which nothing in this process can detect; that half is held by the three syncs, and the power cord is the one test nobody has run."),
        tests: &[crate::anchor::writing_replaces_the_previous_file_only_once_it_is_whole, crate::anchor::two_runs_cannot_hold_the_state_at_once, crate::anchor::a_lock_file_left_behind_by_a_dead_run_is_not_a_lock],
    }),
    statement: NonEmptyStr::new("INV-STATE-004: State is written whole, or the previous one is left intact."),
    rationale: r"A half-written state file claims things that are not true, which is worse than no state file: it will be believed. Written beside the target and moved into place, so an interrupted run leaves the previous state rather than half of two. And two runs cannot hold it at once, because each would write what the other did not know about — held with `flock`, so the kernel drops it whatever ends the process, where a lock file's existence outlived the run that made it and refused every later one until somebody deleted it by hand. A run that only reads takes it shared, which a file's existence cannot express.",
    enforced_by: &[crate::anchor::inv_state_004_1, crate::anchor::inv_state_004_2, crate::anchor::inv_state_004_3, crate::anchor::state_format],
    tests: &[crate::anchor::writing_replaces_the_previous_file_only_once_it_is_whole, crate::anchor::two_runs_cannot_hold_the_state_at_once, crate::anchor::a_lock_file_left_behind_by_a_dead_run_is_not_a_lock, crate::anchor::two_plans_may_read_at_once_and_a_run_that_writes_may_not, crate::anchor::a_file_from_a_version_this_one_does_not_understand_is_refused],
);
