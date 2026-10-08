use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-readme-reflects-registry",
    title: NonEmptyStr::new("The README status line stops saying stage 3 is next"),
    slice: crate::slice::s_readme_reflects_registry,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new(
            "the README status said 'stage 3 next, no cloud yet' after the journal had closed s-stage-3-cloud, and that staleness led an agent to declare a work item under a closed slice"
        ),
    },
    taxon: taxon!(Subsystem, Docs),
    radius: BlastRadius::Local,
    outcome: NonEmptyStr::new(
        "The README status names 0.4.0 as unreleased and the cloud as done, and the roadmap's stage note agrees."
    ),
);
