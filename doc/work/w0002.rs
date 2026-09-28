use crate::taxonomy::Subsystem;
use slipway_core::{taxon, BlastRadius, NonEmptyStr};
use slipway_work::WorkOrigin;

slipway_work::work!(2,
    title: NonEmptyStr::new("The converter writes every decision into the registry, without loss"),
    slice: crate::slice::s0002,
    origin: WorkOrigin::Decision(crate::adr::adr_2026_051),
    taxon: taxon!(Subsystem, Gate),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new(
        "The 48 decision records are written into doc/adr/, each round-tripped byte for byte first, and the registry compiles with them."
    ),
);
