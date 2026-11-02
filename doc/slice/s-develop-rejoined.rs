use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-develop-rejoined",
    title: NonEmptyStr::new("The develop line is joined into main"),
    thrust: crate::thrust::t_hygiene,
    outcome: NonEmptyStr::new(
        "The develop line is joined into main by a true merge, so every landing proof recorded on either line is still valid, and the gate passes on the joined tree."
    ),
    specification: crate::rfc::rfc_2026_001,
    max_radius: BlastRadius::Crate,
);
