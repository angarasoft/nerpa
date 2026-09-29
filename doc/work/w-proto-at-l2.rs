use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-proto-at-l2",
    title: NonEmptyStr::new("The proto is declared at L2"),
    slice: crate::slice::s_subsystems_to_l2,
    origin: WorkOrigin::Decision(crate::adr::adr_2026_061),
    taxon: taxon!(Subsystem, Proto),
    radius: BlastRadius::Local,
    outcome: NonEmptyStr::new("A work item whose radius reaches Proto lands only with a gate proof; the independence a radius requires is a review record."),
);
