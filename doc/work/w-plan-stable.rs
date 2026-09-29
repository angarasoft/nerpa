use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-plan-stable",
    title: NonEmptyStr::new("A recorded configuration does not change its plan without a declared change"),
    slice: crate::slice::s_stage_2_immutable,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("INV-COMPAT-002 is decided but not built: the plan for a recorded configuration must be stable so a change to the engine does not silently rewrite what a fleet believes."),
    },
    taxon: taxon!(Subsystem, Core),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("A recorded configuration whose source does not change produces the same plan; a change appears only when declared."),
);
