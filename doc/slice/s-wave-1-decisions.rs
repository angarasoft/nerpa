use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-wave-1-decisions",
    title: NonEmptyStr::new("Wave 1: decisions move into the registry"),
    thrust: crate::thrust::t_runs_on_dacc,
    outcome: NonEmptyStr::new(
        "Every decision is a registry record, classified against the taxonomy, and the Markdown carrier is gone; cargo xtask ci is green; H8 is held."
    ),
    specification: crate::rfc::rfc_2026_001,
    max_radius: BlastRadius::Crate,
);
