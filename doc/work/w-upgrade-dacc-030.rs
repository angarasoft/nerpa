use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-upgrade-dacc-030",
    title: NonEmptyStr::new("The registry moves onto dacc 0.3.0"),
    slice: crate::slice::s_dacc_030,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("dacc 0.3.0 is published and adds the publication channel to knowledge records, which the registry must declare before drift and survey are available."),
    },
    taxon: taxon!(Subsystem, Pilot),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("The registry compiles against dacc 0.3.0, every knowledge record declares channel::Public, and the CLI is 0.3.0."),
);
