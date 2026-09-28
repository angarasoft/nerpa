use crate::taxonomy::Subsystem;
use slipway_core::{taxon, BlastRadius, NonEmptyStr};
use slipway_work::WorkOrigin;

slipway_work::work!(6,
    title: NonEmptyStr::new("The review type: an independent reading is a record of its own"),
    slice: crate::slice::s0002,
    origin: WorkOrigin::Decision(crate::adr::adr_2026_055),
    taxon: taxon!(Subsystem, Pilot),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new(
        "The local review! type exists, filed at doc/review/, introduced by a decision naming the failure that needed it, with its first record."
    ),
);
