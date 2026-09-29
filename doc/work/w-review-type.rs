use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-review-type",
    title: NonEmptyStr::new("The review type: an independent reading is a record of its own"),
    slice: crate::slice::s_wave_1_decisions,
    origin: WorkOrigin::Decision(crate::adr::adr_2026_055),
    taxon: taxon!(Subsystem, Pilot),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new(
        "The local review! type exists, filed at doc/review/, introduced by a decision naming the failure that needed it, with its first record."
    ),
);
