use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-wave-3-proposals",
    title: NonEmptyStr::new("Wave 3: proposals and invariants"),
    thrust: crate::thrust::t_runs_on_dacc,
    outcome: NonEmptyStr::new(
        "The RFCs are proposals and the invariants are typed records with anchors; no row of DOCUMENTATION.md points at a converted Markdown carrier."
    ),
    specification: crate::rfc::rfc_2026_001,
    max_radius: BlastRadius::Crate,
);
