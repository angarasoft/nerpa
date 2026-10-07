use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-plan-summarises-content",
    title: NonEmptyStr::new("A plan summarises large content rather than printing it whole"),
    thrust: crate::thrust::t_hygiene,
    outcome: NonEmptyStr::new(
        "A plan cuts a text past a hundred or so characters to a prefix and a byte count, and clips long lines the same way, so a megabyte of certificate no longer blows the plan up into something nobody reads."
    ),
    specification: crate::rfc::rfc_2026_002,
    max_radius: BlastRadius::Crate,
);
