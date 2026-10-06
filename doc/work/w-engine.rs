use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-engine",
    title: NonEmptyStr::new("The run is a library the front ends wrap"),
    slice: crate::slice::s_stage_5_surface,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new(
            "Planning and applying are orchestrated inside the CLI beside argument parsing and rendering, so an agent front end would reimplement the run or scrape text (adr-2026-012). adr-2026-073 says the run lives in a nerpa-engine crate the front ends wrap; the CLI becomes a wrapper, and the first machine front end follows over the engine."
        ),
    },
    taxon: taxon!(Subsystem, Core),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new(
        "A nerpa-engine crate plans and applies as a library over the typed Plan, taking the opened Machines as input, and the CLI wraps it instead of holding the orchestration itself."
    ),
);
