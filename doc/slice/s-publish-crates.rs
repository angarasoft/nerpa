use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-publish-crates",
    title: NonEmptyStr::new("The crates publish to crates.io under the name `nerpa`"),
    thrust: crate::thrust::t_runs_on_dacc,
    outcome: NonEmptyStr::new(
        "The name `nerpa` is guarded by publishing, the writing that called the code private is corrected, and cargo-semver-checks guards the gate from the first publish."
    ),
    specification: crate::rfc::rfc_2026_001,
    max_radius: BlastRadius::Local,
);
