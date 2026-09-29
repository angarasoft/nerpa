use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-os-at-l2",
    title: NonEmptyStr::new("The os provider is declared at L2"),
    slice: crate::slice::s_wave_4_l2,
    origin: WorkOrigin::Decision(crate::adr::adr_2026_058),
    taxon: taxon!(Subsystem, Os),
    radius: BlastRadius::Local,
    outcome: NonEmptyStr::new("A work item whose radius reaches Os lands only with a gate proof; the independence a radius requires is a review record."),
);
