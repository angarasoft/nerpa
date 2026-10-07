use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-plan-diff",
    title: NonEmptyStr::new("A changed file prints as a diff, not twice"),
    slice: crate::slice::s_plan_prints_a_diff,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new(
            "W-011: a changed file prints whole, twice, where a diff would print two lines; a one-word change to a hundred-line template prints two hundred lines."
        ),
    },
    taxon: taxon!(Subsystem, Cli),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new(
        "The plan's rendering of a changed file computes a line diff of the two sides — trimming the common prefix and suffix and marking the changed middle `-` and `+` — and the one difference a line diff cannot show, the final newline, is said outright."
    ),
);
