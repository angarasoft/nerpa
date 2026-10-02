use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-dacc-050",
    title: NonEmptyStr::new("The registry moves onto dacc 0.5.0"),
    thrust: crate::thrust::t_runs_on_dacc,
    outcome: NonEmptyStr::new(
        "The registry compiles against dacc 0.5.0, whose record model carries obligations — what executes a contract — and the gate runs the CLI at 0.5.0."
    ),
    specification: crate::rfc::rfc_2026_001,
    max_radius: BlastRadius::Crate,
);
