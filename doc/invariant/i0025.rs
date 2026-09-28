use crate::invariant::{Enforced, Status};
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-REACH-001"),
    status: Status::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("One machine not answering refusing the whole run, or its resources being planned anyway from state that is out of date. Taken once, and not where the guarantee was tested: connections are opened for the whole fleet before anything is read, and a failure there ended the run without the planner being asked."), tests: &[crate::anchor::a_resource_on_a_machine_that_did_not_answer_is_not_planned, crate::anchor::nothing_at_all_is_said_about_what_could_not_be_read, crate::anchor::a_connection_that_fails_before_anything_is_read_does_not_end_the_run, crate::anchor::a_removal_on_a_machine_that_did_not_answer_is_not_planned, crate::anchor::a_removal_waiting_on_one_that_cannot_be_made_is_held] }),
    statement: NonEmptyStr::new("A machine that did not answer is refused per resource, not per run."),
    rationale: r"Refusing the whole run because one machine is down blocks changes to every other machine, which is how people learn to work around a tool. Everything readable is planned; what could not be read appears saying so, with the machine and the age of the newest knowledge, and nothing is claimed about it. See 0020.",
    enforced_by: &[crate::anchor::inv_reach_001_1, crate::anchor::planner_removals, crate::anchor::planner_walk, crate::anchor::run_observe],
    tests: &[crate::anchor::a_resource_on_a_machine_that_did_not_answer_is_not_planned, crate::anchor::nothing_at_all_is_said_about_what_could_not_be_read, crate::anchor::a_machine_not_answering_lets_the_reading_continue, crate::anchor::a_connection_that_fails_before_anything_is_read_does_not_end_the_run, crate::anchor::the_machine_that_would_not_open_is_named_once_and_says_why, crate::anchor::a_fleet_where_nothing_opens_is_empty_rather_than_fatal, crate::anchor::a_removal_on_a_machine_that_did_not_answer_is_not_planned, crate::anchor::a_removal_waiting_on_one_that_cannot_be_made_is_held],
);
