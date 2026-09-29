use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-registry-skeleton",
    title: NonEmptyStr::new("The registry skeleton: doc/ crate, taxonomy, and the pilot's first records"),
    slice: crate::slice::s_wave_0_switch,
    origin: WorkOrigin::Decision(crate::adr::adr_2026_050),
    taxon: taxon!(Subsystem, Pilot),
    radius: BlastRadius::Local,
    outcome: NonEmptyStr::new(
        "doc/ is a workspace member; the taxonomy declares the subsystems and the team; the pilot's decision, specification, thrust, slice and this work item compile and reference each other by path."
    ),
);
