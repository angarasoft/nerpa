use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-semver-gate",
    title: NonEmptyStr::new("cargo-semver-checks guards the gate from the first publish"),
    thrust: crate::thrust::t_runs_on_dacc,
    outcome: NonEmptyStr::new(
        "cargo-semver-checks runs in `cargo xtask ci` and in CI, so a breaking change to a published API is a gate failure."
    ),
    specification: crate::rfc::rfc_2026_001,
    max_radius: BlastRadius::Local,
);
