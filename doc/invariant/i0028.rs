use crate::invariant::{Enforced, Status};
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-REACH-004"),
    status: Status::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("Reporting a held resource through its own machine, or only when it has one. A change on a machine that answered reads as that machine's silence, and a held control-domain change disappears from the plan."), tests: &[crate::anchor::a_cause_waits_while_what_it_rouses_cannot_be_planned_and_has_no_record] }),
    statement: NonEmptyStr::new("What waits for something that cannot be planned is said to wait for it, whatever its domain, and only a machine that did not answer is reported as one."),
    rationale: r"Everything a plan could not speak for was listed as out of reach on its own machine. A change held on a machine that answered, because what it reads from, reacts with or removes was on one that did not, was reported as that machine not answering, with an age from records seconds old; and a change on no machine was not reported at all, so a plan could leave a cloud change out and name only the service it was waiting for. Waiting is listed apart, with what it waits for, and a plan with either list is incomplete.",
    enforced_by: &[crate::anchor::inv_reach_004_1, crate::anchor::inv_reach_004_2, crate::anchor::planner_removals],
    tests: &[crate::anchor::a_resource_that_reads_from_an_unreachable_one_cannot_be_planned_either, crate::anchor::a_cause_waits_while_what_it_rouses_cannot_be_planned_and_has_no_record, crate::anchor::a_held_cause_on_no_machine_is_named_all_the_same, crate::anchor::a_held_removal_is_said_to_wait_for_its_root, crate::anchor::a_removal_held_behind_a_machine_is_said_to_wait_for_it_not_for_its_successor, crate::anchor::a_held_removal_names_the_root_of_a_longer_wait, crate::anchor::a_change_waiting_on_one_that_cannot_be_planned_says_what_for],
);
