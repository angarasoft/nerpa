use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-stage-5-surface",
    title: NonEmptyStr::new("Stage 5 — the machine-facing surface"),
    thrust: crate::thrust::t_stage_5_surface,
    outcome: NonEmptyStr::new(
        "An agent plans and applies a change end to end under a grant, with a human signing the hash; a deliberately over-reaching request is refused by the grant rather than by the agent's judgement."
    ),
    specification: crate::rfc::rfc_2026_003,
    max_radius: BlastRadius::Crate,
);
