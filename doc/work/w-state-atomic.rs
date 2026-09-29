use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-state-atomic",
    title: NonEmptyStr::new("State is written whole, or the previous one is left intact"),
    slice: crate::slice::s_stage_2_immutable,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("INV-STATE-004 is claimed but not enforced: the bypass — a torn state from a partial write — is described but not yet attacked."),
    },
    taxon: taxon!(Subsystem, Core),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("A state write that fails leaves the previous state intact, never a torn one."),
);
