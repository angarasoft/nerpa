use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-stage-4-access",
    title: NonEmptyStr::new("Stage 4 — who is allowed to do what"),
    thrust: crate::thrust::t_stage_4_access,
    outcome: NonEmptyStr::new(
        "A run carries an actor identity and refuses without one; static keys are not a mode; one party applies and another signs; the log verifies from the first entry."
    ),
    specification: crate::rfc::rfc_2026_003,
    max_radius: BlastRadius::Crate,
);
