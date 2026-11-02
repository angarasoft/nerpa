use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-rust-1991",
    title: NonEmptyStr::new("The toolchain moves onto Rust 1.99.1"),
    slice: crate::slice::s_rust_1991,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("The tree pins 1.99.0 and 1.99.1, the point release of the same line, is out; a toolchain bump is upkeep, and the plan hash's reproducibility wants the pin moved deliberately rather than left to rustup"),
    },
    taxon: taxon!(Subsystem, Gate),
    radius: BlastRadius::Local,
    outcome: NonEmptyStr::new("rust-toolchain.toml pins 1.99.1, the README names it, the workspace MSRV stays 1.99, and the full gate passes on it."),
);
