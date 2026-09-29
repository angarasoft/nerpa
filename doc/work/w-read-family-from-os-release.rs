use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-read-family-from-os-release",
    title: NonEmptyStr::new("The package test reads the os family, not the package manager binary"),
    slice: crate::slice::s_maintenance,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("the test guessed the family from whether rpm or portageq was installed, and a machine with rpm but a debian os was refused"),
    },
    taxon: taxon!(Subsystem, Os),
    radius: BlastRadius::Local,
    outcome: NonEmptyStr::new("The package read-back test skips where the os family is not rhel-like or gentoo, instead of guessing from the installed binary."),
);
