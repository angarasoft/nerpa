use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-stage-6-others",
    title: NonEmptyStr::new("Stage 6 — other people's work"),
    thrust: crate::thrust::t_stage_6_others,
    outcome: NonEmptyStr::new(
        "A provider built outside this repository loads, creates a resource, and demonstrably cannot read a credential value or reach a host outside its manifest."
    ),
    specification: crate::rfc::rfc_2026_002,
    max_radius: BlastRadius::Crate,
);
