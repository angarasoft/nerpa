use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-import-state",
    title: NonEmptyStr::new("The import records the resources it mapped, as state"),
    slice: crate::slice::s_stage_3_cloud,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new(
            "A configuration alone is not enough for an empty plan: a resource declared but not recorded is adopted on the next plan, and adoption is a change. The import must also write state, so the first plan has nothing to do."
        ),
    },
    taxon: taxon!(Subsystem, Cli),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new(
        "The import produces a state that records each mapped resource with its attributes as plain values, and no secret among them."
    ),
);
