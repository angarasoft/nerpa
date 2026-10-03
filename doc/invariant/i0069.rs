use dacc_knowledge::{Enforced, InvariantStatus};
use dacc_core::NonEmptyStr;

dacc_knowledge::invariant!("i0069",
    specification: crate::rfc::rfc_2026_001,
    status: InvariantStatus::Enforced(Enforced::Adversarial {
        bypass: NonEmptyStr::new("A subject typed without a Dacc-Work or Dacc-Slice trailer, or carrying one that names a work item outside this tree."),
        tests: &[crate::anchor::a_basis_less_message_is_refused, crate::anchor::a_basis_naming_no_work_item_is_refused],
    }),
    statement: NonEmptyStr::new("INV-PILOT-003: Every commit after the switch carries a basis naming a work item in the commit's own tree."),
    rationale: r"The commit gate reads the message and the tree, not the habit: a message without its trailer is refused, and so is one whose trailer names a record this tree does not hold. The same rule runs twice on purpose — dacc's msg-check in CI and the gate here — and both are shown a message that breaks it.",
    enforced_by: &[crate::anchor::the_commit_gate_checks_the_basis],
    tests: &[crate::anchor::a_basis_less_message_is_refused, crate::anchor::a_basis_naming_no_work_item_is_refused],
);
