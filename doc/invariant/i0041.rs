use dacc_knowledge::{Enforced, InvariantStatus};
use dacc_core::NonEmptyStr;

dacc_knowledge::invariant!("i0041",
    specification: crate::rfc::rfc_2026_002,
    status: InvariantStatus::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("Taking the bound off again, because nothing needs it until stage 3 and everything compiles without it until then. That is the whole risk: it is a promise about an interface that no caller depends on yet, so nothing but this entry and the test beside it notices its absence."), tests: &[crate::anchor::one_provider_serves_two_threads_at_once] }),
    statement: NonEmptyStr::new("INV-PROVIDER-001: One provider serves every machine, so the interface is shareable across threads."),
    rationale: r"Stage 3 visits machines in parallel and one registry serves all of them, so a provider is used from several threads at once. The bound sits on the trait rather than on each implementation because the trait is what people outside this repository write against, and it is added now because there are none of them yet: after the repository opens, adding it breaks somebody else's code and they are not in the room to argue. See decision 0031.",
    enforced_by: &[crate::anchor::provider_trait],
    tests: &[crate::anchor::a_file_is_written_and_the_second_run_has_nothing_to_do],
);
