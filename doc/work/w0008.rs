use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!(8,
    title: NonEmptyStr::new("The registry re-pins onto dacc, Slipway renamed"),
    slice: crate::slice::s0002,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new(
            "dacc is Slipway renamed, published on crates.io; the registry re-pins onto it and renames the carrier."
        ),
    },
    taxon: taxon!(Subsystem, Gate),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new(
        "dacc-core, dacc-knowledge, dacc-work, dacc-scan and dacc-derive come from crates.io; the config, hooks, CI, prose and trailers are renamed; cargo dacc gate is green."
    ),
);
