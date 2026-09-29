use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-wave-2-plan",
    title: NonEmptyStr::new("Wave 2: the plan"),
    thrust: crate::thrust::t_runs_on_dacc,
    outcome: NonEmptyStr::new(
        "The product's current work — the seven divergences and the roadmap's open stages — is in the registry."
    ),
    specification: crate::rfc::rfc_2026_001,
    max_radius: BlastRadius::Crate,
);
