use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-real-tests",
    title: NonEmptyStr::new("The built system is exercised on real machines, not only in unit tests"),
    thrust: crate::thrust::t_runs_on_dacc,
    outcome: NonEmptyStr::new(
        "A repeatable harness runs the engine, the wire and the os provider against real machines over ssh, and proves convergence, an empty second run, and a change that restarts the service that reads it."
    ),
    specification: crate::rfc::rfc_2026_001,
    max_radius: BlastRadius::Crate,
);
