use dacc_knowledge::{Enforced, InvariantStatus};
use dacc_core::NonEmptyStr;

dacc_knowledge::invariant!("i0033",
    specification: crate::rfc::rfc_2026_002,
    status: InvariantStatus::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("A provider handed a credential recovered from state, which is only possible if state held one."), tests: &[crate::anchor::a_password_reaches_the_file_and_nothing_else] }),
    statement: NonEmptyStr::new("INV-SECRET-003: A provider is never handed a secret out of state, because state never held one."),
    rationale: r"Deleting or relocating a resource reads its attributes back from state, and those attributes are how the provider finds it. A secret is not a locator — nothing is found by its password — so `Record::locator` yields the values and drops the digests. Anything that genuinely needs the credential fetches it from the store, which is the point of not keeping it.",
    enforced_by: &[crate::anchor::inv_secret_003],
    tests: &[crate::anchor::a_password_reaches_the_file_and_nothing_else],
);
