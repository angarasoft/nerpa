use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-readme-reflects-registry",
    title: NonEmptyStr::new("The README and roadmap say what the registry says"),
    thrust: crate::thrust::t_runs_on_dacc,
    outcome: NonEmptyStr::new(
        "The README status line names the real version and the cloud as done, not stage 3 as next."
    ),
    specification: crate::rfc::rfc_2026_001,
    max_radius: BlastRadius::Local,
);
