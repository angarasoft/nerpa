use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-state-secrets",
    title: NonEmptyStr::new("State holds references to secrets, never their values"),
    slice: crate::slice::s_stage_2_immutable,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("INV-STATE-001 is decided but not built: state must hold a reference and a digest of a secret, never the value, so a leaked state file leaks nothing."),
    },
    taxon: taxon!(Subsystem, Core),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("A secret's value appears nowhere in the state file; the state holds a reference and a digest, and the resource still converges."),
);
