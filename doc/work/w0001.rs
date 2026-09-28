use crate::taxonomy::Subsystem;
use slipway_core::{taxon, BlastRadius, NonEmptyStr};
use slipway_work::WorkOrigin;

slipway_work::work!(1,
    title: NonEmptyStr::new("The registry skeleton: doc/ crate, taxonomy, and the pilot's first records"),
    slice: crate::slice::s0001,
    origin: WorkOrigin::Decision(crate::adr::adr_2026_050),
    taxon: taxon!(Subsystem, Pilot),
    radius: BlastRadius::Local,
    outcome: NonEmptyStr::new(
        "doc/ is a workspace member; the taxonomy declares the subsystems and the team; the pilot's decision, specification, thrust, slice and this work item compile and reference each other by path."
    ),
);
