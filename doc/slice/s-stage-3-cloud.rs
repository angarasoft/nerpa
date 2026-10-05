use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-stage-3-cloud",
    title: NonEmptyStr::new("Stage 3 — the cloud, and a way in from Terraform"),
    thrust: crate::thrust::t_stage_3_cloud,
    outcome: NonEmptyStr::new(
        "One real environment moves across without recreating a single resource; nerpa plan immediately after import is empty; import reports every secret it found in clear text in the old state; recorded-exchange tests cover the provider."
    ),
    specification: crate::rfc::rfc_2026_002,
    max_radius: BlastRadius::Crate,
);
