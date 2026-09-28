use crate::taxonomy::Subsystem;
use slipway_core::{taxon, BlastRadius, NonEmptyStr};
use slipway_work::WorkOrigin;

slipway_work::work!(4,
    title: NonEmptyStr::new("Links and code references point at the registry"),
    slice: crate::slice::s0002,
    origin: WorkOrigin::Decision(crate::adr::adr_2026_051),
    taxon: taxon!(Subsystem, Gate),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new(
        "Every link and code reference names a decision by number, resolved against doc/adr/; cargo xtask docs and book read the registry, and the cost-is-named check holds."
    ),
);
