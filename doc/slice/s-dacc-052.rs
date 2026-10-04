use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-dacc-052",
    title: NonEmptyStr::new("The registry rides on dacc 0.5.2"),
    thrust: crate::thrust::t_runs_on_dacc,
    outcome: NonEmptyStr::new(
        "The registry compiles against dacc 0.5.2, and the gate installs and runs the CLI at the version Cargo.toml pins."
    ),
    specification: crate::rfc::rfc_2026_001,
    max_radius: BlastRadius::Crate,
);
