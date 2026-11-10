use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-born-node-refused",
    title: NonEmptyStr::new("A node a cloud resource creates is refused unless the fleet routes it"),
    thrust: crate::thrust::t_stage_3_cloud,
    outcome: NonEmptyStr::new(
        "A node a cloud resource creates is reached only by a route the fleet names, and is refused by name otherwise."
    ),
    specification: crate::rfc::rfc_2026_002,
    max_radius: BlastRadius::Crate,
);
