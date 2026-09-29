use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-pilot-verdict",
    title: NonEmptyStr::new("The verdict on whether dacc earned adoption, read independently"),
    slice: crate::slice::s_pilot_verdict,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new(
            "The verdict is a reading of the nine hypotheses against the records that measured them, recorded as a review; it changes no product code and carries no architectural trace."
        ),
    },
    taxon: taxon!(Subsystem, Pilot),
    radius: BlastRadius::Local,
    outcome: NonEmptyStr::new(
        "The verdict is recorded against the criteria written before the start; each H1–H9 is listed with its result and the record that measured it; the verdict is one of continue, narrow or stop."
    ),
);
