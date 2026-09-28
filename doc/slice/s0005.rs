use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!(5,
    title: NonEmptyStr::new("Wave 2: the plan"),
    thrust: crate::thrust::t0001,
    outcome: NonEmptyStr::new(
        "The product's current work — the seven divergences and the roadmap's open stages — is in the registry."
    ),
    specification: crate::rfc::rfc_2026_001,
    max_radius: BlastRadius::Crate,
);
