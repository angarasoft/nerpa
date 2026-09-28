use slipway_core::{BlastRadius, NonEmptyStr};

slipway_work::slice!(1,
    title: NonEmptyStr::new("Wave 0: the switch onto Slipway's rules"),
    thrust: crate::thrust::t0001,
    outcome: NonEmptyStr::new(
        "The registry builds under nerpa's lints; the gate and msg-check run on one protected branch; every commit after the switch carries a basis; every pilot record existed before the switch."
    ),
    specification: crate::rfc::rfc_2026_001,
    max_radius: BlastRadius::Crate,
);
