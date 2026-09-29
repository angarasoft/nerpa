use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-upgrade-dacc",
    title: NonEmptyStr::new("The registry moves onto dacc 0.2.0 slugs"),
    slice: crate::slice::s_wave_4_l2,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("dacc 0.2.0 is published, and the registry migrates from numeric ids to slugs"),
    },
    taxon: taxon!(Subsystem, Pilot),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("The registry compiles against dacc 0.2.0, where work, slice and thrust ids are slugs."),
);
