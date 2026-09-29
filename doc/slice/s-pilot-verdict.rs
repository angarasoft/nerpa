use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-pilot-verdict",
    title: NonEmptyStr::new("The verdict: the remaining hypotheses are measured and dacc is judged against the criteria written before the start"),
    thrust: crate::thrust::t_runs_on_dacc,
    outcome: NonEmptyStr::new(
        "H3, H5, H6, H7 and a re-check of H8 are measured against the tree, the verdict on whether dacc earned adoption is recorded against the criteria written before the start, and the reading is a review record."
    ),
    specification: crate::rfc::rfc_2026_001,
    max_radius: BlastRadius::Crate,
);
