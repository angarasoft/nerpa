use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-publish-readiness",
    title: NonEmptyStr::new("The public record carries no external artifacts, and the README states how the project is governed"),
    thrust: crate::thrust::t_runs_on_dacc,
    outcome: NonEmptyStr::new(
        "The review records name no external models, and the README says how nerpa is governed — decisions, specifications, invariants and work as compiled code."
    ),
    specification: crate::rfc::rfc_2026_001,
    max_radius: BlastRadius::Crate,
);
