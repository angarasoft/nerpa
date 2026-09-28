use crate::invariant::Status;
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-COMPAT-002"),
    status: Status::Planned,
    statement: NonEmptyStr::new("The plan for a recorded configuration does not change without the change being declared."),
    rationale: r#"The plan is the compatibility surface, and a corpus of configurations with their plans checked in makes that promise something a machine verifies rather than something a person intends. A version that changes one either has a bug or has made a decision, and the changelog entry is what failed the build until written."#,
    enforced_by: &[],
    tests: &[],
);
