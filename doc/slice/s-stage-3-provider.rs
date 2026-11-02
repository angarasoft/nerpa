use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-stage-3-provider",
    title: NonEmptyStr::new("Stage 3 — a Yandex Cloud provider in the control domain"),
    thrust: crate::thrust::t_stage_3_cloud,
    outcome: NonEmptyStr::new(
        "A control-domain provider manages a set of Yandex Cloud resources from a Starlark configuration, converging with recorded-exchange tests that cover it."
    ),
    specification: crate::rfc::rfc_2026_002,
    max_radius: BlastRadius::Crate,
);
