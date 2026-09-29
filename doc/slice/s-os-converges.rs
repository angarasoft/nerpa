use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-os-converges",
    title: NonEmptyStr::new("The os provider converges: three divergences"),
    thrust: crate::thrust::t_converges_to_invariants,
    outcome: NonEmptyStr::new(
        "The three os divergences — a service whose unset enabled/running disables it on update, a removal that stops and disables what it never declared, and a text file mode that never converges — are closed against the baseline specification."
    ),
    specification: crate::rfc::rfc_2026_002,
    max_radius: BlastRadius::Local,
);
