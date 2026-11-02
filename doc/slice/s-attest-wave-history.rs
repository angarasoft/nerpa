use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-attest-wave-history",
    title: NonEmptyStr::new("The wave history is attested and closed"),
    thrust: crate::thrust::t_runs_on_dacc,
    outcome: NonEmptyStr::new(
        "The eleven remaining wave works are landed from history, and the four wave slices close."
    ),
    specification: crate::rfc::rfc_2026_001,
    max_radius: BlastRadius::Crate,
);
