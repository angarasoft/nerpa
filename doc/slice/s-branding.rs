use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-branding",
    title: NonEmptyStr::new("Branding: the project has a logo and a favicon"),
    thrust: crate::thrust::t_runs_on_dacc,
    outcome: NonEmptyStr::new(
        "The repository carries a generated icon set, the README and the book show the logo, and the built book serves the favicon."
    ),
    specification: crate::rfc::rfc_2026_001,
    max_radius: BlastRadius::Local,
);
