use dacc_knowledge::{Enforced, InvariantStatus};
use dacc_core::NonEmptyStr;

dacc_knowledge::invariant!("i0063",
    specification: crate::rfc::rfc_2026_002,
    status: InvariantStatus::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("A rename across a provider, a kind or a slot is refused, and a rename without a `moved` declaration reads as a removal and a creation after it, as INV-PLAN-009 says."), tests: &[crate::anchor::a_declared_rename_keeps_its_identity] }),
    statement: NonEmptyStr::new("INV-PLAN-013: A resource renamed with `moved` keeps its identity: the plan proposes no destruction and no recreation, and the run moves the record to the new address."),
    rationale: r"The address is the identity of a resource (decision 0002), so renaming `os:file.old` to `os:file.new` read as a delete and a create — which for a database, a disk or a machine is a very expensive way to improve a name. A `moved` declaration makes the rename a migration (decision 0028): the plan reports no change for the renamed resource, the run moves the record and remembers the migration, and a later plan against the same state knows the migration is spent.",
    enforced_by: &[crate::anchor::inv_plan_013, crate::anchor::run_apply],
    tests: &[crate::anchor::a_declared_rename_keeps_its_identity],
);
