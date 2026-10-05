use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-one-branch",
    title: NonEmptyStr::new("The repository works on one branch"),
    thrust: crate::thrust::t_runs_on_dacc,
    outcome: NonEmptyStr::new(
        "Work lands on `main`, the `develop` branch is gone, and the contributor notes and CI say so."
    ),
    specification: crate::rfc::rfc_2026_001,
    max_radius: BlastRadius::Local,
);
