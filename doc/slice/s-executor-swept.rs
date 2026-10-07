use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-executor-swept",
    title: NonEmptyStr::new("A machine holds one executor, not one per build"),
    thrust: crate::thrust::t_hygiene,
    outcome: NonEmptyStr::new(
        "Delivering a new executor sweeps the ones it replaces, so a long-lived machine holds a single executor rather than one per build it was ever converged by."
    ),
    specification: crate::rfc::rfc_2026_002,
    max_radius: BlastRadius::Crate,
);
