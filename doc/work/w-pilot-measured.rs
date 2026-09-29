use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-pilot-measured",
    title: NonEmptyStr::new("The remaining pilot hypotheses are measured and recorded"),
    slice: crate::slice::s_pilot_verdict,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("Core reached L2, so the verdict is due; H1, H2, H4, H8 and H9 are already measured, and H3, H5, H6 and H7 need the numbers drawn from the tree before a verdict can be read."),
    },
    taxon: taxon!(Subsystem, Pilot),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("H3, H5, H6, H7 and a re-check of H8 are measured against the tree and recorded as the review v0011."),
);
