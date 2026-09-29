use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-converter",
    title: NonEmptyStr::new("The converter writes every decision into the registry, without loss"),
    slice: crate::slice::s_wave_1_decisions,
    origin: WorkOrigin::Decision(crate::adr::adr_2026_051),
    taxon: taxon!(Subsystem, Gate),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new(
        "The 48 decision records are written into doc/adr/, each round-tripped byte for byte first, and the registry compiles with them."
    ),
);
