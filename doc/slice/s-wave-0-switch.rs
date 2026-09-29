use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-wave-0-switch",
    title: NonEmptyStr::new("Wave 0: the switch onto dacc's rules"),
    thrust: crate::thrust::t_runs_on_dacc,
    outcome: NonEmptyStr::new(
        "The registry builds under nerpa's lints; the gate and msg-check run on one protected branch; every commit after the switch carries a basis; every pilot record existed before the switch."
    ),
    specification: crate::rfc::rfc_2026_001,
    max_radius: BlastRadius::Crate,
);
