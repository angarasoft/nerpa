use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-dacc-030",
    title: NonEmptyStr::new("The registry moves onto dacc 0.3.0"),
    thrust: crate::thrust::t_runs_on_dacc,
    outcome: NonEmptyStr::new(
        "The registry compiles against dacc 0.3.0, where every knowledge record declares its publication channel."
    ),
    specification: crate::rfc::rfc_2026_001,
    max_radius: BlastRadius::Crate,
);
