use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-schema-kinds",
    title: NonEmptyStr::new("The Schema gains the monotonic and stop-to-change kinds"),
    slice: crate::slice::s_stage_3_provider,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("The cloud needs two answers the Schema does not have yet. A disk can only grow, never shrink, and a running instance's cores and memory cannot change in place — the cloud refuses unless the machine is stopped first. Both are what-a-difference-means, so both belong in the Schema rather than in a provider's guess."),
    },
    taxon: taxon!(Subsystem, Core),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("The Schema declares monotonic (a value may only grow, a shrink is refused) and stop-to-change (changing the value requires the resource to be stopped first), with tests."),
);
