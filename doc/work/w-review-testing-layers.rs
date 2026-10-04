use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-review-testing-layers",
    title: NonEmptyStr::new("The testing layers say what runs, and what is planned"),
    slice: crate::slice::s_review_0013,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new(
            "review v0013's major-1 and minor-3 are prose that disagrees with the tree: two named layers do not exist and one command is spelled wrong. Rewriting the document is routine, with no architectural trace left behind"
        ),
    },
    taxon: taxon!(Subsystem, Docs),
    radius: BlastRadius::Local,
    outcome: NonEmptyStr::new(
        "docs/TESTING.md names the layers the suite runs and marks the missing ones planned, naming the work that would build each; the running block says `cargo xtask mutants`, the single entry point."
    ),
);
