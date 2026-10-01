use crate::invariant::{Enforced, Status};
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-COMPAT-002"),
    status: Status::Enforced(Enforced::Adversarial {
        bypass: NonEmptyStr::new("a change to the plan output of a recorded configuration with no changelog entry; the corpus compares the current plan to the checked-in golden and fails until the golden is updated and a changelog entry is written."),
        tests: &[crate::anchor::compat_rename],
    }),
    statement: NonEmptyStr::new("The plan for a recorded configuration does not change without the change being declared."),
    rationale: r"The plan is the compatibility surface, and a corpus of configurations with their plans checked in makes that promise something a machine verifies rather than something a person intends. A version that changes one either has a bug or has made a decision, and the changelog entry is what failed the build until written.",
    enforced_by: &[crate::anchor::compat_check],
    tests: &[crate::anchor::compat_create, crate::anchor::compat_update, crate::anchor::compat_delete, crate::anchor::compat_rename],
);
