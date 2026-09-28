use crate::invariant::{Enforced, Status};
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-PLAN-010"),
    status: Status::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("Asking only about the removals this plan carries, as INV-PLAN-009 does. The declared resource is adopted from the reading and loses its thing when the removal comes."), tests: &[crate::anchor::a_resource_naming_what_a_held_removal_takes_is_held_with_it] }),
    statement: NonEmptyStr::new("A declared resource naming what a held removal takes away is held with it."),
    rationale: r"A removal that cannot be planned yet, because its machine did not answer or it waits for one that did not, still takes its thing away in whichever later run it can. A declared resource naming the same thing, planned in the meantime, was adopted from the reading, and that later run removed the file from under its record. It waits for the removal, and the plan says so.",
    enforced_by: &[crate::anchor::planner_walk],
    tests: &[crate::anchor::a_resource_naming_what_a_held_removal_takes_is_held_with_it],
);
