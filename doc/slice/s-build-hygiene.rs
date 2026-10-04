use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-build-hygiene",
    title: NonEmptyStr::new("Build hygiene: the build output lives beside the sources"),
    thrust: crate::thrust::t_runs_on_dacc,
    outcome: NonEmptyStr::new(
        "The workspace build writes into `.cargo-target` beside the sources and `.gitignore` carries it, so one `cargo sweep` cleans the whole of it; the gate's exported tree builds beside its own copy rather than into a shared `target/`."
    ),
    specification: crate::rfc::rfc_2026_001,
    max_radius: BlastRadius::Local,
);
