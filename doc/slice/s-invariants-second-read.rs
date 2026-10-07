use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-invariants-second-read",
    title: NonEmptyStr::new("The invariants promoted by one hand pass a second read"),
    thrust: crate::thrust::t_hygiene,
    outcome: NonEmptyStr::new(
        "Each of the thirteen invariants W-012 names is read by a second hand: its adversarial test attacks the bypass its own entry names, and the one that did not — INV-SECRET-003 — has the missing attempt written and the entry pointing at it."
    ),
    specification: crate::rfc::rfc_2026_002,
    max_radius: BlastRadius::Crate,
);
