use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-remove-md-carrier",
    title: NonEmptyStr::new("The Markdown carrier is removed"),
    slice: crate::slice::s_wave_1_decisions,
    origin: WorkOrigin::Decision(crate::adr::adr_2026_051),
    taxon: taxon!(Subsystem, Gate),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new(
        "docs/decisions/ is gone, the converter that moved the records is gone, and cargo xtask ci is green with the registry as the single carrier."
    ),
);
