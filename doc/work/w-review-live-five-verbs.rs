use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-review-live-five-verbs",
    title: NonEmptyStr::new("The live harness drives the five verbs of stage 4"),
    slice: crate::slice::s_review_0013,
    origin: WorkOrigin::Divergence {
        specification: crate::rfc::rfc_2026_003,
        limitation: crate::limitation::l_harness_runs_three_verbs,
    },
    taxon: taxon!(Subsystem, Gate),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new(
        "`cargo xtask live --stage4` runs plan --json, sign as a second party with a key of its own, apply over the approval, an out-of-band change, a second apply refused naming what moved, and journal verify — marked as needing the live park. The convergence check matches the whole line, and the executor is installed through a name of each run's own."
    ),
);
