use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-repo-hygiene",
    title: NonEmptyStr::new("The emptied transition plan is gone and what it held is enrolled"),
    slice: crate::slice::s_repo_hygiene,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("the transition plan emptied itself into the registry as its records landed, and its own text says the work item that finishes moving a file there deletes it; the stale mutants output at the root is tool litter"),
    },
    taxon: taxon!(Subsystem, Docs),
    radius: BlastRadius::Local,
    outcome: NonEmptyStr::new("slipway_plan/ and the stale mutants output are gone; the plan's live risks and the study's findings sit in their registry homes; nothing points at a file that no longer exists."),
);
