use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-merge-needs-person",
    title: NonEmptyStr::new("A resource marked for merging is never overwritten without a person"),
    slice: crate::slice::s_stage_2_immutable,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("INV-DRIFT-001 is decided but not built: drift on a resource marked for merging must stop the stage and ask for a person, rather than silently correcting a hand-edited file."),
    },
    taxon: taxon!(Subsystem, Core),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("A resource marked for merging that has drifted stops the stage and asks for a person rather than being silently overwritten."),
);
