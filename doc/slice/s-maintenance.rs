use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-maintenance",
    title: NonEmptyStr::new("Maintenance: the gate keeps the tree's version, and the landing page reads well"),
    thrust: crate::thrust::t_runs_on_dacc,
    outcome: NonEmptyStr::new("The gate installs the dacc tool at the tree's pinned version, and the landing page presents the project."),
    specification: crate::rfc::rfc_2026_001,
    max_radius: BlastRadius::Local,
);
