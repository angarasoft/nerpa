use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-ci-msg-check",
    title: NonEmptyStr::new("The commit-message gate checks the slug era and nothing before it"),
    thrust: crate::thrust::t_runs_on_dacc,
    outcome: NonEmptyStr::new(
        "The CI commit-message check never reaches before the slug convention (2274c575), so a push that brings the whole history passes."
    ),
    specification: crate::rfc::rfc_2026_001,
    max_radius: BlastRadius::Local,
);
