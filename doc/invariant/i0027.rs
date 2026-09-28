use crate::invariant::{Enforced, Status};
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-REACH-003"),
    status: Status::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("A plan that could not read everything presenting itself as complete, and so as signable."), tests: &[crate::anchor::what_could_not_be_looked_at_is_reported_with_its_age_and_never_as_work] }),
    statement: NonEmptyStr::new("A plan that could not look at everything is not complete, and says so."),
    rationale: r"Nobody can approve a change to infrastructure they were unable to see. Such a plan carries an age, an age moves every second, and a hash over it never settles — so it cannot be signed. Stated by name rather than left as an unexplained hash mismatch, because that is a mystery at three in the morning.",
    enforced_by: &[crate::anchor::inv_reach_003_1, crate::anchor::inv_reach_003_2],
    tests: &[crate::anchor::what_could_not_be_looked_at_is_reported_with_its_age_and_never_as_work, crate::anchor::the_age_of_what_is_known_comes_from_state],
);
