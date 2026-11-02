use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-stage-3-decomposition",
    title: NonEmptyStr::new("Stage 3 is enrolled with its three slices"),
    slice: crate::slice::s_stage_3_provider,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("The stage 3 thrust was empty. The cloud is a provider in the control domain, a way in from Terraform schemas, and an import that moves an environment across without recreation; the three are enrolled as slices so the thrust is no longer empty."),
    },
    taxon: taxon!(Subsystem, Core),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("Slices s-stage-3-provider, s-stage-3-schema and s-stage-3-import sit under t-stage-3-cloud."),
);
