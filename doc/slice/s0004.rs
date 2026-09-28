use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!(4,
    title: NonEmptyStr::new("The core converges: four divergences"),
    thrust: crate::thrust::t0007,
    outcome: NonEmptyStr::new(
        "The four core divergences — a stale attribute reference, a lost machine stopping a whole read, a world-readable state file, and a control-domain resource disappearing from the plan — are closed against the baseline specification."
    ),
    specification: crate::rfc::rfc_2026_002,
    max_radius: BlastRadius::Crate,
);
