use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-build-executor-for-budget",
    title: NonEmptyStr::new("The budget run builds the executor beside nerpa"),
    slice: crate::slice::s_maintenance,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("the budget step built only nerpa, and nerpa plan needs its executor beside it"),
    },
    taxon: taxon!(Subsystem, Gate),
    radius: BlastRadius::Local,
    outcome: NonEmptyStr::new("The budget run builds both nerpa and nerpa-executor."),
);
