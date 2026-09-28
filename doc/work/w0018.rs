use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!(18,
    title: NonEmptyStr::new("The proposal type, and the RFCs as proposals"),
    slice: crate::slice::s0006,
    origin: WorkOrigin::Decision(crate::adr::adr_2026_056),
    taxon: taxon!(Subsystem, Pilot),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new(
        "The local proposal! type exists, and RFCs 0001 to 0003 are p0001 to p0003 with bodies, review rounds as review records, and decided parts checked against decisions."
    ),
);
