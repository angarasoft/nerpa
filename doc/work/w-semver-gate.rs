use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-semver-gate",
    title: NonEmptyStr::new("cargo-semver-checks joins `ci`"),
    slice: crate::slice::s_semver_gate,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new(
            "The first crates are published, so the baseline cargo-semver-checks compares against now exists, and adr-2026-014 says it joins the gate once anything is published."
        ),
    },
    taxon: taxon!(Subsystem, Gate),
    radius: BlastRadius::Local,
    outcome: NonEmptyStr::new(
        "`cargo xtask ci` runs cargo-semver-checks over the workspace, and CI installs the tool and runs the same gate."
    ),
);
