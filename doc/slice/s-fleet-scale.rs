use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-fleet-scale",
    title: NonEmptyStr::new("The 1000-machine design gaps are named"),
    thrust: crate::thrust::t_stage_3_cloud,
    outcome: NonEmptyStr::new(
        "The missing pieces for managing a thousand machines — a rollout axis and a failure policy — are recorded in the watchlist, tied to stage 3."
    ),
    specification: crate::rfc::rfc_2026_002,
    max_radius: BlastRadius::Local,
);
