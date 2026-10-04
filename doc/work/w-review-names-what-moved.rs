use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-review-names-what-moved",
    title: NonEmptyStr::new("The refusal names everything its number covers"),
    slice: crate::slice::s_review_0013,
    origin: WorkOrigin::Divergence {
        specification: crate::rfc::rfc_2026_003,
        limitation: crate::limitation::l_refusal_names_nothing,
    },
    taxon: taxon!(Subsystem, Cli),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new(
        "what_moved diffs the migrations from and to, the merges, and the order and composition of the stages — everything the plan number covers beyond who acts and the changes address by address — and a refusal whose diff comes out empty names both numbers rather than trailing a dash."
    ),
);
