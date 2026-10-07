use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-executor-sweep",
    title: NonEmptyStr::new("Delivery sweeps the executors it replaces"),
    slice: crate::slice::s_executor_swept,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new(
            "W-008: nothing ever deletes an executor a machine has stopped needing, so a long-lived machine accumulates one per build it was ever converged by."
        ),
    },
    taxon: taxon!(Subsystem, Transport),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new(
        "The delivery copy that puts a new executor under its name sweeps every other `nerpa-executor-*` beside it, so the machine holds the latest alone."
    ),
);
