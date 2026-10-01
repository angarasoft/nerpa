use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-repo-hygiene",
    title: NonEmptyStr::new("Repository hygiene: the emptied plan is gone and what it held is enrolled"),
    thrust: crate::thrust::t_runs_on_dacc,
    outcome: NonEmptyStr::new(
        "slipway_plan/ and the stale mutants output are gone from the tree; the transition plan's live risks and the study's findings sit in their registry homes; nothing in the tree points at a file that no longer exists."
    ),
    specification: crate::rfc::rfc_2026_001,
    max_radius: BlastRadius::Local,
);
