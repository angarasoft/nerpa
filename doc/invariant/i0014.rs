use crate::invariant::{Enforced, Status};
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-STATE-003"),
    status: Status::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("A moved resource looked for where the declaration now points, found missing, and made a second time."), tests: &[crate::anchor::moving_a_file_replaces_it_and_takes_the_old_one_with_it] }),
    statement: NonEmptyStr::new("A resource that moved is found where it was, not where it should be."),
    rationale: r#"Reading at the new declared location finds nothing and proposes making a second resource, abandoning the first. Only state remembers where Nerpa left it, and without that a change of location is silently a duplication."#,
    enforced_by: &[crate::anchor::inv_state_003_2, crate::anchor::run_observe],
    tests: &[crate::anchor::moving_a_file_replaces_it_and_takes_the_old_one_with_it, crate::anchor::a_file_moved_onto_a_path_a_removal_frees_leaves_nothing_at_the_old_path],
);
