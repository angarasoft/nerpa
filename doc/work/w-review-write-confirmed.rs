use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-review-write-confirmed",
    title: NonEmptyStr::new("A change to the corpus names its changelog in the same commit"),
    slice: crate::slice::s_review_0013,
    origin: WorkOrigin::Divergence {
        specification: crate::rfc::rfc_2026_004,
        limitation: crate::limitation::l_changelog_is_convention,
    },
    taxon: taxon!(Subsystem, Gate),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new(
        "The gate refuses a commit that changes crates/nerpa-core/tests/corpus/ without a CHANGELOG.md hunk in it, shown a fixture that breaks it; INV-COMPAT-002 names both holders. Rule 7 has an executor for its third sentence: unwrap_or_default() outside tests is refused, and the rules read past inline test modules instead of stopping at the first one."
    ),
);
