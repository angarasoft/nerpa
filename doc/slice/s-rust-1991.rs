use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-rust-1991",
    title: NonEmptyStr::new("The toolchain moves onto Rust 1.99.1"),
    thrust: crate::thrust::t_runs_on_dacc,
    outcome: NonEmptyStr::new(
        "rust-toolchain.toml pins 1.99.1 and the README names it, the workspace MSRV stays 1.99, and the gate runs on it."
    ),
    specification: crate::rfc::rfc_2026_001,
    max_radius: BlastRadius::Local,
);
