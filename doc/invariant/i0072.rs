use dacc_knowledge::{Enforced, InvariantStatus};
use dacc_core::NonEmptyStr;

dacc_knowledge::invariant!("i0072",
    specification: crate::rfc::rfc_2026_001,
    status: InvariantStatus::Enforced(Enforced::Adversarial {
        bypass: NonEmptyStr::new("A record whose context lost the sections the conversion carried, read by a checker that never looks."),
        tests: &[crate::anchor::a_converted_record_keeps_its_sections_in_context],
    }),
    statement: NonEmptyStr::new("INV-PILOT-006: Converting a Markdown record to the registry and back loses nothing: the sections compare byte for byte."),
    rationale: r"The byte-for-byte comparison is v0001's measurement, made while the converter existed and both carriers stood; it is a measured past. What remains enforceable is its residue — the sections live in the record's context field, none loses its reasoning — and that is the site marked here and the thing the test looks at.",
    enforced_by: &[crate::anchor::the_sections_live_in_the_records_context],
    tests: &[crate::anchor::a_converted_record_keeps_its_sections_in_context],
);
