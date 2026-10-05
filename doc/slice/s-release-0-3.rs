use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-release-0-3",
    title: NonEmptyStr::new("Release 0.3.0: tag the stage 3 boundary"),
    thrust: crate::thrust::t_runs_on_dacc,
    outcome: NonEmptyStr::new(
        "Version 0.3.0 is tagged at the stage 3 boundary, the tree's version says 0.3.0, and the changelog marks what 0.3.0 carries."
    ),
    specification: crate::rfc::rfc_2026_001,
    max_radius: BlastRadius::Local,
);
