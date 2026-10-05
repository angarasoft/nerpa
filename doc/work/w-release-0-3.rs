use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-release-0-3",
    title: NonEmptyStr::new("Release 0.3.0: the stage 3 boundary is tagged"),
    slice: crate::slice::s_release_0_3,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new(
            "Stage 3 closed, so its boundary is released: the tree's version moves to 0.3.0, the changelog marks what 0.3.0 carries, and a tag names the boundary (adr-2026-014)."
        ),
    },
    taxon: taxon!(Subsystem, Docs),
    radius: BlastRadius::Local,
    outcome: NonEmptyStr::new(
        "The tree's version is 0.3.0, the changelog marks 0.3.0, and the tag v0.3.0 sits on that commit."
    ),
);
