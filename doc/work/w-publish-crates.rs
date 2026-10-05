use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-publish-crates",
    title: NonEmptyStr::new("The crates publish to crates.io, and the gate guards the name"),
    slice: crate::slice::s_publish_crates,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new(
            "A crate name is first-come-first-served, and decision 0019 says the name is unguarded until it is published. The main crate is named `nerpa-cli`, which would guard `nerpa-cli` but not `nerpa`; it is renamed. Publishing makes the source public, so the writing that called the repository private is corrected, and adr-2026-014 says cargo-semver-checks joins the gate once anything is published."
        ),
    },
    taxon: taxon!(Subsystem, Gate),
    radius: BlastRadius::Local,
    outcome: NonEmptyStr::new(
        "The main crate is named `nerpa`, the writing no longer calls the code private, cargo-semver-checks is reachable through the gate, and the crates publish to crates.io in dependency order."
    ),
);
