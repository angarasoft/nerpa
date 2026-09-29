use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-subsystems-to-l2",
    title: NonEmptyStr::new("Each subsystem is declared at L2: its work lands only with a gate proof"),
    thrust: crate::thrust::t_runs_on_dacc,
    outcome: NonEmptyStr::new(
        "Core, Config, Proto, Executor, Transport, Cli, Gate and Docs are each declared at L2 — a work item whose radius reaches one lands only with a gate proof, and the independence a radius requires is a review record."
    ),
    specification: crate::rfc::rfc_2026_001,
    max_radius: BlastRadius::Crate,
);
