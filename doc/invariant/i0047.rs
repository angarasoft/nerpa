use crate::invariant::{Enforced, Status};
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-PLAN-009"),
    status: Status::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("A name known only later, as for INV-PLAN-008, or a record whose naming attribute is not a plain value. Either plans the declared resource from the reading again."), tests: &[crate::anchor::a_resource_renamed_at_the_same_path_converges_in_one_run] }),
    statement: NonEmptyStr::new("A declared resource naming what a removal, or a move away, takes away is made again after it."),
    rationale: r#"A removal is planned from state and a declared resource from a reading, and when both name one thing on a machine the reading describes something this plan is about to delete. Renaming an address and keeping its path, with no `moved`, removed the file and adopted the new resource from the reading taken before, so the run ended with no file and a record saying there was one. Until 0028 builds `moved`, a rename that keeps what it names is a removal and a creation after it, and the plan says both."#,
    enforced_by: &[crate::anchor::inv_plan_009_3, crate::anchor::planner_edges, crate::anchor::planner_walk],
    tests: &[crate::anchor::a_resource_naming_what_a_removal_takes_is_created_after_it, crate::anchor::a_resource_renamed_at_the_same_path_converges_in_one_run, crate::anchor::a_file_moved_onto_a_path_a_removal_frees_leaves_nothing_at_the_old_path, crate::anchor::a_file_declared_where_a_replacement_moves_one_away_is_made_after_it],
);
