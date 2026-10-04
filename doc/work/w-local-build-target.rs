use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-local-build-target",
    title: NonEmptyStr::new("The local build output lives in .cargo-target, cleaned by cargo sweep"),
    slice: crate::slice::s_build_hygiene,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new(
            "where the build output goes is a convention, not architecture: pointing it beside the sources and ignoring it is routine, and it leaves no trace in the product code"
        ),
    },
    taxon: taxon!(Subsystem, Gate),
    radius: BlastRadius::Local,
    outcome: NonEmptyStr::new(
        "`.cargo/config.toml` sets `target-dir = \".cargo-target\"` and `.gitignore` carries `.cargo-target/`, so the build output sits beside the sources instead of in `target/`; `cargo sweep` cleans the whole of it."
    ),
);
