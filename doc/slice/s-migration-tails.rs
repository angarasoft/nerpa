use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-migration-tails",
    title: NonEmptyStr::new("The 0.6.0 migration's tails"),
    thrust: crate::thrust::t_runs_on_dacc,
    outcome: NonEmptyStr::new(
        "The norm marker is marked on demand and the policy is recorded, and the orphaned anchors are reconciled with the records that reference them."
    ),
    specification: crate::rfc::rfc_2026_001,
    max_radius: BlastRadius::Crate,
);
