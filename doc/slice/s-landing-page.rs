use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-landing-page",
    title: NonEmptyStr::new("The landing page reads for a newcomer"),
    thrust: crate::thrust::t_runs_on_dacc,
    outcome: NonEmptyStr::new(
        "The README leads a newcomer in: an example that creates a machine and configures the file on it, the core ideas as links into the book, and a place next to the tools a reader arrives from."
    ),
    specification: crate::rfc::rfc_2026_001,
    max_radius: BlastRadius::Local,
);
