use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-presentable-readme",
    title: NonEmptyStr::new("The landing page presents the project"),
    slice: crate::slice::s_maintenance,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("the landing page lacked badges, a quick start and links to the documentation"),
    },
    taxon: taxon!(Subsystem, Docs),
    radius: BlastRadius::Local,
    outcome: NonEmptyStr::new("The README carries badges, a quick start, and links to the documentation."),
);
