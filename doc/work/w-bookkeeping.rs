use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-bookkeeping",
    title: NonEmptyStr::new("The book-keeping catches up with the tree"),
    slice: crate::slice::s_stage_2_immutable,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("the wave history was never journaled, the status line still says stage 2 is next although its outcome holds, and the boundary between doc/ and docs/ was left undescribed; a plan whose state disagrees with its tree is the drift the registry exists to remove"),
    },
    taxon: taxon!(Subsystem, Docs),
    radius: BlastRadius::Local,
    outcome: NonEmptyStr::new("The wave works' history is journaled and s-wave-0 to s-wave-3 are closed; README and CHANGELOG say stages 0 through 2 are complete; MAP and DOCUMENTATION name where the RFC bodies live and how the registry is navigated; W-027 is closed."),
);
