use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-grant",
    title: NonEmptyStr::new("A grant limits a run, and the engine refuses what it does not allow"),
    slice: crate::slice::s_stage_5_surface,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new(
            "The actor's scope is a free string nothing reads yet; adr-2026-009 says a grant limits a run to named kinds, targets and operations, with an expiry, and is checked by the engine rather than by the thing being limited. Stage 5 begins by making that real."
        ),
    },
    taxon: taxon!(Subsystem, Core),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new(
        "A grant names the kinds, targets and operations a run may touch, and the engine refuses a plan whose changes reach outside it."
    ),
);
