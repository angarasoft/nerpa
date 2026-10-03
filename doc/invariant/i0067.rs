use dacc_knowledge::{Enforced, InvariantStatus};
use dacc_core::NonEmptyStr;

dacc_knowledge::invariant!("i0067",
    specification: crate::rfc::rfc_2026_001,
    status: InvariantStatus::Enforced(Enforced::Unrepresentable),
    statement: NonEmptyStr::new("INV-PILOT-001: A reference to a record that no longer exists does not compile."),
    rationale: r"A reference between records is a path to a constant the scan generates from the registry at build time, so a record naming a document that is gone does not resolve and the build stops. The marked site is the generator shared by every kind of record; what the types cannot see — a reference naming a record of another kind than the field asks for — the walk beside it sees.",
    enforced_by: &[crate::anchor::a_reference_is_a_path_to_a_generated_constant],
    tests: &[crate::anchor::every_reference_names_a_record_of_its_kind],
);
