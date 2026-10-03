use dacc_knowledge::{Enforced, InvariantStatus};
use dacc_core::NonEmptyStr;

dacc_knowledge::invariant!("i0050",
    specification: crate::rfc::rfc_2026_002,
    status: InvariantStatus::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("Comparing a mode as text, or accepting any octal number. A mode written another way, or with the file type bits, is changed again on every run."), tests: &[crate::anchor::a_mode_is_compared_as_the_number_it_spells] }),
    statement: NonEmptyStr::new("INV-PLAN-012: A file mode is written as text and compared as the number it spells; a number, or text that spells no mode a file can have, is refused while planning."),
    rationale: r#"A mode was compared as the text it was written in. Written "644" or "0o644", it never matched the "0644" a reading reports, and one beyond the permission bits, such as "100644", never matched at all, because a machine keeps only those bits. Each was planned as an update on every run. A provider says which attributes are modes; the planner compares them as numbers and refuses, before anything is done, one that is not octal or goes beyond the permission bits."#,
    enforced_by: &[crate::anchor::inv_plan_012_2, crate::anchor::planner_differences],
    tests: &[crate::anchor::a_mode_is_compared_as_the_number_it_spells, crate::anchor::a_mode_beyond_the_permission_bits_is_refused_while_planning, crate::anchor::a_mode_written_as_a_number_is_refused_and_says_to_quote_it],
);
