use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-one-branch",
    title: NonEmptyStr::new("Work lands on one branch, and `develop` is gone"),
    slice: crate::slice::s_one_branch,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new(
            "Work lands as gated commits (adr-2026-014: a slice ends with a commit, and a branch that lives long is a slice cut too large). A `develop` branch adds a merge dance that the gate and the registry already make redundant, so the repository works on `main` and a release is a tag at a stage boundary."
        ),
    },
    taxon: taxon!(Subsystem, Gate),
    radius: BlastRadius::Local,
    outcome: NonEmptyStr::new(
        "The contributor notes say work lands on `main`, CI builds `main`, and `develop` is deleted everywhere."
    ),
);
