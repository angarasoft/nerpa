use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-state-file-private",
    title: NonEmptyStr::new("The state file is not readable by everybody"),
    slice: crate::slice::s_core_converges,
    origin: WorkOrigin::Divergence {
        specification: crate::rfc::rfc_2026_002,
        limitation: crate::limitation::l_state_keeps_no_values,
    },
    taxon: taxon!(Subsystem, Core),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("The state file, digest key included, is written readable only by its owner."),
);
