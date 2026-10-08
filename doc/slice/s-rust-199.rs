use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-rust-199",
    title: NonEmptyStr::new("The toolchain and MSRV move onto Rust 1.99"),
    thrust: crate::thrust::t_runs_on_dacc,
    outcome: NonEmptyStr::new(
        "rust-toolchain.toml pins 1.99 and the workspace MSRV names 1.99, and the gate runs on it."
    ),
    specification: crate::rfc::rfc_2026_001,
    max_radius: BlastRadius::Local,
);
