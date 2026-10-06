use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-gate-upkeep",
    title: NonEmptyStr::new("The gate's budget and live harnesses stay in step with the tree"),
    thrust: crate::thrust::t_runs_on_dacc,
    outcome: NonEmptyStr::new(
        "The budget and live tasks read built binaries from the directory `.cargo/config.toml` names, and the budget names its actor, so both harnesses run against the tree as it now stands."
    ),
    specification: crate::rfc::rfc_2026_001,
    max_radius: BlastRadius::Local,
);
