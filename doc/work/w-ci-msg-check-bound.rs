use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-ci-msg-check-bound",
    title: NonEmptyStr::new("The commit-message check starts at the slug convention"),
    slice: crate::slice::s_ci_msg_check,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new(
            "The commit-message gate's lower bound named the dacc transition (ba215f4), but commits between it and the slug convention — dacc 0.2.0 — carry a numeric work id, `Dacc-Work: w0017`, which msg-check refuses. The bound moves to where the slug form begins (2274c575), so a push that brings the whole history passes."
        ),
    },
    taxon: taxon!(Subsystem, Gate),
    radius: BlastRadius::Local,
    outcome: NonEmptyStr::new(
        "The CI commit-message check clamps its range to 2274c575, the first commit in the slug form, and `cargo dacc msg-check --range 2274c575..HEAD` is clean."
    ),
);
