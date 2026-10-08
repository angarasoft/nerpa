use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-rust-199",
    title: NonEmptyStr::new("The toolchain and MSRV move onto Rust 1.99"),
    slice: crate::slice::s_rust_199,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("Rust 1.99 is the current stable and the tree pins 1.98.1; a toolchain bump is upkeep, and the plan hash's reproducibility wants the pin moved deliberately rather than left to rustup"),
    },
    taxon: taxon!(Subsystem, Gate),
    radius: BlastRadius::Local,
    outcome: NonEmptyStr::new("rust-toolchain.toml pins 1.99, the workspace MSRV names 1.99, and the full gate passes on it."),
);
