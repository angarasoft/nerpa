use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-stage-3-import",
    title: NonEmptyStr::new("Stage 3 — an environment moves across without recreation"),
    thrust: crate::thrust::t_stage_3_cloud,
    outcome: NonEmptyStr::new(
        "A real environment's tfstate imports into nerpa without recreating a single resource; the plan immediately after import is empty; import reports every secret it found in clear text in the old state."
    ),
    specification: crate::rfc::rfc_2026_002,
    max_radius: BlastRadius::Crate,
);
