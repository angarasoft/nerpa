use dacc_knowledge::{Enforced, InvariantStatus};
use dacc_core::NonEmptyStr;

dacc_knowledge::invariant!("i0068",
    specification: crate::rfc::rfc_2026_001,
    status: InvariantStatus::Enforced(Enforced::Unrepresentable),
    statement: NonEmptyStr::new("INV-PILOT-002: A work item cannot be landed without a gate proof recorded on the same tree."),
    rationale: r"A landing is an event that carries its proof: the gate verdict and the tree hash it was measured on, or the history it was restored from and says so. There is no shape of landing that claims a gate and carries no verdict. The fold refuses an event the state machine cannot accept, and the walk beside it checks the one thing no field ties together: that the gate and the landing share their tree. A landing restored from history carries no gate by design and is marked evidence = history.",
    enforced_by: &[crate::anchor::the_journal_fold_refuses_a_landing_without_its_gate],
    tests: &[crate::anchor::every_gated_landing_carries_its_gate_on_the_same_tree],
);
