use crate::invariant::{Enforced, Status};
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-STATE-002"),
    status: Status::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("A removal ordered from the configuration, which by definition no longer describes the thing being removed."), tests: &[crate::anchor::what_depended_on_a_thing_is_removed_before_the_thing, crate::anchor::what_came_after_a_thing_is_removed_before_the_thing, crate::anchor::what_read_from_a_thing_that_already_existed_is_removed_before_the_thing, crate::anchor::an_edge_added_to_a_resource_that_already_matches_is_remembered] }),
    statement: NonEmptyStr::new("A resource is removed before whatever it depended on."),
    rationale: r#"Removal order cannot come from the configuration, because the configuration that described the dependency is exactly what was deleted. It comes from the edges state recorded, and a resource that already matches is recorded again when its edges change, or an `after` written in later never reaches state. The other way round takes a network down before the machine on it."#,
    enforced_by: &[crate::anchor::inv_state_002_3, crate::anchor::planner_change_for, crate::anchor::planner_edges],
    tests: &[crate::anchor::what_depended_on_a_thing_is_removed_before_the_thing, crate::anchor::what_came_after_a_thing_is_removed_before_the_thing, crate::anchor::what_read_from_a_thing_that_already_existed_is_removed_before_the_thing, crate::anchor::an_edge_added_to_a_resource_that_already_matches_is_remembered, crate::anchor::a_resource_given_a_new_edge_is_reordered_and_nothing_else, crate::anchor::a_resource_removed_from_the_configuration_is_removed_from_the_machine],
);
