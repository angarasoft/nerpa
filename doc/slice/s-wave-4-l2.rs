use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-wave-4-l2",
    title: NonEmptyStr::new("Wave 4: the L2 declaration and the migration"),
    thrust: crate::thrust::t_runs_on_dacc,
    outcome: NonEmptyStr::new(
        "The first subsystem is declared L2, and the registry moves onto the dacc version that carries the gate proof."
    ),
    specification: crate::rfc::rfc_2026_001,
    max_radius: BlastRadius::Crate,
);
