use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-architecture-audited",
    title: NonEmptyStr::new("Every partly-built marker says what is really unbuilt"),
    thrust: crate::thrust::t_runs_on_dacc,
    outcome: NonEmptyStr::new(
        "Each of the sixteen partly-built sections is read against the tree: the two whose gap is closed say built, and the fourteen whose gap is open are kept."
    ),
    specification: crate::rfc::rfc_2026_001,
    max_radius: BlastRadius::Local,
);
