use dacc_knowledge::{Enforced, InvariantStatus};
use dacc_core::NonEmptyStr;

dacc_knowledge::invariant!("i0042",
    specification: crate::rfc::rfc_2026_002,
    status: InvariantStatus::Enforced(Enforced::Adversarial {
        bypass: NonEmptyStr::new("A provider that reaches the machine from `prepare` or `conclude` — sending its own operations, so the engine cannot group, order or count what it never sees. The trait hands neither method a connection, so this cannot be written against it; a provider that spawns its own process is out of scope, because no signature can prevent that, and the claim here is only that the engine owns the journey."),
        tests: &[crate::anchor::the_engine_sends_what_the_provider_prepared, crate::anchor::a_replacement_is_forgotten_between_the_half_that_removes_and_the_half_that_makes],
    }),
    statement: NonEmptyStr::new("INV-PROVIDER-002: A provider reaches a machine only to read: the engine sends what a change is made of."),
    rationale: r"A provider that opens its own journey makes the batch unrepresentable. The engine cannot group, order or count what it never sees, and `INV-BATCH-001` exists to keep one journey per machine rather than one per resource. So `prepare` is handed no connection, `conclude` is handed answers rather than a machine, and the only `Reach` left on the trait is the one `read` needs. See RFC 0001 §5.2 and decision 0044.",
    enforced_by: &[crate::anchor::inv_provider_002_2, crate::anchor::provider_trait],
    tests: &[crate::anchor::the_engine_sends_what_the_provider_prepared, crate::anchor::a_replacement_is_forgotten_between_the_half_that_removes_and_the_half_that_makes],
);
