use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!(6,
    title: NonEmptyStr::new("Wave 3: proposals and invariants"),
    thrust: crate::thrust::t0001,
    outcome: NonEmptyStr::new(
        "The RFCs are proposals and the invariants are typed records with anchors; no row of DOCUMENTATION.md points at a converted Markdown carrier."
    ),
    specification: crate::rfc::rfc_2026_001,
    max_radius: BlastRadius::Crate,
);
