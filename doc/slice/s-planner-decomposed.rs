use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-planner-decomposed",
    title: NonEmptyStr::new("The planner's one file becomes four, each one responsibility"),
    thrust: crate::thrust::t_hygiene,
    outcome: NonEmptyStr::new(
        "planner.rs is split along its responsibilities — the single pass, the removal ordering, the per-resource change decision and the naming helpers — so a reviewer holds one of them at a time, and the behaviour does not move."
    ),
    specification: crate::rfc::rfc_2026_002,
    max_radius: BlastRadius::Crate,
);
