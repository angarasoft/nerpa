use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-stage-2-immutable",
    title: NonEmptyStr::new("Stage 2 — what cannot be changed later"),
    thrust: crate::thrust::t_stage_2_immutable,
    outcome: NonEmptyStr::new(
        "A password appears in neither the state file nor the plan and the resource still converges; a resource renamed keeps its identity and is not rebuilt; the six open invariants reach Enforced."
    ),
    specification: crate::rfc::rfc_2026_002,
    max_radius: BlastRadius::Crate,
);
