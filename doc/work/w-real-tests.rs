use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-real-tests",
    title: NonEmptyStr::new("The built system is exercised on real machines, not only in unit tests"),
    slice: crate::slice::s_real_tests,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("the gate proves only the unit tests; nothing runs the engine, the wire and the os provider against a real machine"),
    },
    taxon: taxon!(Subsystem, Cli),
    radius: BlastRadius::Local,
    outcome: NonEmptyStr::new("A repeatable harness runs the engine, the wire and the os provider against real machines over ssh, and proves convergence, an empty second run, and a change that restarts the service that reads it."),
);
