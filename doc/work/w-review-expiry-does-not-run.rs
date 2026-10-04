use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-review-expiry-does-not-run",
    title: NonEmptyStr::new("An identity whose expiry passed does not run"),
    slice: crate::slice::s_review_0013,
    origin: WorkOrigin::Divergence {
        specification: crate::rfc::rfc_2026_003,
        limitation: crate::limitation::l_expiry_is_decorative,
    },
    taxon: taxon!(Subsystem, Core),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new(
        "Actor::new refuses an `--as-expiry` already in the past against the current time, and one that is not an RFC3339 time at all, each refusal naming what it refused; an expiry in the future is carried as before. The scope stays carried and unchecked until grants exist, and INV-TRUST-001 says which half moved."
    ),
);
