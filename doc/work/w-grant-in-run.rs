use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-grant-in-run",
    title: NonEmptyStr::new("A run carries a grant, and apply refuses what it does not allow"),
    slice: crate::slice::s_stage_5_surface,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new(
            "The grant is a type and a check but not in the run: the actor's scope is still a free string, and apply does not look at it. adr-2026-009 says a grant limits a run and is checked by the engine, so the actor carries the grant and apply refuses a plan whose changes reach outside it."
        ),
    },
    taxon: taxon!(Subsystem, Core),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new(
        "A run carries a grant in its actor, the grant is part of the plan number, and apply refuses a change that reaches outside it."
    ),
);
