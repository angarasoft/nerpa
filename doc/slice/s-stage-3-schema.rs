use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-stage-3-schema",
    title: NonEmptyStr::new("Stage 3 — Terraform schemas become the resource types"),
    thrust: crate::thrust::t_stage_3_cloud,
    outcome: NonEmptyStr::new(
        "A Terraform provider schema in JSON becomes nerpa's description of resource types, taking the schemas and leaving the provider binaries."
    ),
    specification: crate::rfc::rfc_2026_002,
    max_radius: BlastRadius::Crate,
);
