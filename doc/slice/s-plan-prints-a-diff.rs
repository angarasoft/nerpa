use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-plan-prints-a-diff",
    title: NonEmptyStr::new("A plan prints a diff of a changed file, not the file twice"),
    thrust: crate::thrust::t_hygiene,
    outcome: NonEmptyStr::new(
        "A changed file in a plan is printed as a line diff — the common prefix and suffix trimmed, the changed middle marked `-` and `+` — instead of both sides whole, so a one-line change to a hundred-line file prints a handful of lines."
    ),
    specification: crate::rfc::rfc_2026_002,
    max_radius: BlastRadius::Crate,
);
