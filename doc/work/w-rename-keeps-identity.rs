use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-rename-keeps-identity",
    title: NonEmptyStr::new("A resource renamed keeps its identity and is not rebuilt"),
    slice: crate::slice::s_stage_2_immutable,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("the stage-2 exit criterion: a rename is a change of address, not of existence, so a resource renamed in the configuration keeps its identity and the plan proposes no destruction and no recreation."),
    },
    taxon: taxon!(Subsystem, Core),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("Renaming a resource in the configuration keeps its identity; the plan proposes no destruction and no recreation."),
);
