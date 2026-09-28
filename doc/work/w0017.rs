use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!(17,
    title: NonEmptyStr::new("The CI's msg-check starts after the transition onto dacc"),
    slice: crate::slice::s0005,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new(
            "The first push to an empty repository is the whole history, and msg-check refuses what predates the transition: no basis at all, or a Slipway-Work trailer and the old type list."
        ),
    },
    taxon: taxon!(Subsystem, Gate),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new(
        "The CI's msg-check range starts at ba215f4, the first commit carrying a Dacc-Work basis and the final type list."
    ),
);
