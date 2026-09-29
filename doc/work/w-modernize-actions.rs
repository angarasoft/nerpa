use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-modernize-actions",
    title: NonEmptyStr::new("The gate uses non-deprecated actions and a pinned runner"),
    slice: crate::slice::s_maintenance,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("the gate ran on actions targeting node 20 and an unpinned runner, both deprecated"),
    },
    taxon: taxon!(Subsystem, Gate),
    radius: BlastRadius::Local,
    outcome: NonEmptyStr::new("The gate runs on checkout@v5, upload-artifact@v5 and ubuntu-24.04."),
);
