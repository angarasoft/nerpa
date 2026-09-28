use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!(16,
    title: NonEmptyStr::new("The roadmap's open stages and the seven divergences move into the registry"),
    slice: crate::slice::s0005,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new(
            "The plan: stages 2 to 6 of the roadmap become thrusts, a baseline specification names the invariants the first slices touch, and the seven open defects become Divergence work items in two slices."
        ),
    },
    taxon: taxon!(Subsystem, Pilot),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new(
        "The thrusts t0002 to t0007, the baseline specification rfc-2026-002, the slices s0003 and s0004, and the seven Divergence work items w0009 to w0015 are in the registry."
    ),
);
