use dacc_knowledge::{Enforced, InvariantStatus};
use dacc_core::NonEmptyStr;

dacc_knowledge::invariant!("i0015",
    specification: crate::rfc::rfc_2026_002,
    status: InvariantStatus::Enforced(Enforced::Adversarial {
        bypass: NonEmptyStr::new("A secret's value recorded as Plain — a path that fetches the value and hands it to state without sealing it first — so the state file carries the password in clear text."),
        tests: &[crate::anchor::a_password_reaches_the_file_and_nothing_else, crate::anchor::a_recorded_secret_shows_that_it_is_one_and_nothing_more],
    }),
    statement: NonEmptyStr::new("INV-STATE-001: State holds references to secrets and digests of their values, never the values."),
    rationale: r"Terraform state carries passwords in clear text, which is why it needs whole-file encryption and still leaks through every backup. A state that never held the value has nothing to leak: a record holds `Recorded`, whose secret variant is a keyed digest with no path back to a value, and the accessor a provider reads through yields the values and drops the digests.",
    enforced_by: &[crate::anchor::inv_secret_003],
    tests: &[crate::anchor::a_password_reaches_the_file_and_nothing_else, crate::anchor::a_recorded_secret_shows_that_it_is_one_and_nothing_more],
);
