use crate::invariant::{Enforced, Status};
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-SECRET-001"),
    status: Status::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("A secret reaching the accessor that rendering, state and the plan all read through, and being written down by one of them."), tests: &[crate::anchor::a_recorded_secret_shows_that_it_is_one_and_nothing_more] }),
    statement: NonEmptyStr::new("A secret attribute never reports a value through the accessor everything that writes things down goes through."),
    rationale: r#"`Attribute::known` is what every part of the engine that persists or prints reaches for, so a secret answering `None` there makes the guarantee structural rather than a rule each caller has to remember. The value exists — somewhere else, behind a `Source` — and reaching it takes a deliberate fetch."#,
    enforced_by: &[crate::anchor::inv_secret_001],
    tests: &[crate::anchor::a_recorded_secret_shows_that_it_is_one_and_nothing_more, crate::anchor::a_password_reaches_the_file_and_nothing_else],
);
