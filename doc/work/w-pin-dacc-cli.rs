use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-pin-dacc-cli",
    title: NonEmptyStr::new("The gate installs the dacc tool at the tree's pinned version"),
    slice: crate::slice::s_maintenance,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("the gate installed the latest dacc-cli, which leapt ahead of the tree's pinned version and broke msg-check"),
    },
    taxon: taxon!(Subsystem, Gate),
    radius: BlastRadius::Local,
    outcome: NonEmptyStr::new("The gate installs dacc-cli at the version Cargo.toml pins, not the latest on crates.io."),
);
