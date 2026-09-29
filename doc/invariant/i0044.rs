use crate::invariant::{Enforced, Status};
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-PLAN-006"),
    status: Status::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("Anything a person approves that is not inside the number: a field added to a change and not to the encoding, or a catch-all arm folding an unknown variant into a default. Both leave a signature covering a plan that is not the one that runs, and neither shows up as a failure — only as two plans with one number."), tests: &[crate::anchor::every_part_of_a_change_moves_the_number, crate::anchor::text_that_runs_together_does_not_collide, crate::anchor::a_plan_that_could_not_look_at_everything_has_no_number, crate::anchor::two_values_that_differ_never_get_the_same_number] }),
    statement: NonEmptyStr::new("A plan has one number covering everything it intends to do, and a plan that could not look at everything has none."),
    rationale: r"Three things rest on it and each is worth exactly what it is worth. `0033` lets a batch of operations run anything, and what limits that authority is the plan it came from. `0034` binds the premise to it. `0009` has a third party sign it.

It covers the intent: the stages, and every change in them — the address, the kind, the whole desired state and every delta. Not what the plan could not see, and not what it merely reports; those belong to the premise, which covers what the plan was computed *from*. Between the two the whole plan is accounted for and neither is doing the other's job.

A plan carrying an unreachable machine has none at all. Such a plan carries how old that knowledge is, an age moves every second, and a number over it would never settle — so it says why instead of offering one that cannot be compared twice. `INV-REACH-003` already refuses to call such a plan complete.

The encoding is written out rather than borrowed from a serialisation format, carries a domain prefix, and has no catch-all arm anywhere: adding a variant to any of the enums it walks stops the build, which is the right place to be asked how it hashes rather than having two different plans quietly share a number.",
    enforced_by: &[crate::anchor::inv_plan_006_1, crate::anchor::value_encode],
    tests: &[crate::anchor::the_same_plan_is_the_same_number_every_time, crate::anchor::a_plan_with_nothing_in_it_still_has_one, crate::anchor::an_empty_list_and_an_empty_map_are_different_numbers],
);
