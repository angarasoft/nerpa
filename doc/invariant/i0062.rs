use crate::invariant::{Enforced, Status};
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-DELIVERY-001"),
    status: Status::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("Deciding from whether a file is executable, or moving a copy into place because the command that wrote it succeeded. A cut-off input then leaves a short executor under the right name for good."), tests: &[crate::anchor::an_interrupted_copy_is_never_put_under_the_executors_name] }),
    statement: NonEmptyStr::new("An executor is taken as delivered only when the whole of it is under its name."),
    rationale: r"Delivery writes the executor beside its name and moves it into place, so that a copy cut short is never taken for a complete one. The copy was `cat`, which takes an input ending early for the end of the file: an `ssh` killed part way left a short copy, the script moved it under the name, and the file test that decides whether to deliver passed on every run after, while the short binary died without a word. The copy is counted before it is moved, under a name of its own for each attempt, and what is already there is counted too.",
    enforced_by: &[crate::anchor::inv_delivery_001_1, crate::anchor::inv_delivery_001_2],
    tests: &[crate::anchor::an_interrupted_copy_is_never_put_under_the_executors_name, crate::anchor::a_short_copy_already_there_is_not_taken_for_the_executor],
);
