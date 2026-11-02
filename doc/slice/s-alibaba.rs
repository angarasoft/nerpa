use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-alibaba",
    title: NonEmptyStr::new("Stage 3 — an Alibaba Cloud provider in the control domain"),
    thrust: crate::thrust::t_stage_3_cloud,
    outcome: NonEmptyStr::new(
        "A control-domain provider manages Alibaba Cloud VPCs, vSwitches and ECS instances from a Starlark configuration, reached with a short-lived session and never a long-lived AccessKey, converging with recorded-exchange tests that cover it."
    ),
    specification: crate::rfc::rfc_2026_002,
    max_radius: BlastRadius::Crate,
);
