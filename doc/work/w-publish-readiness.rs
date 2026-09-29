use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-publish-readiness",
    title: NonEmptyStr::new("The records are anonymized and the governance is documented"),
    slice: crate::slice::s_publish_readiness,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("the gate's external-names check, once given a list, flagged the review records naming external models — so the names are anonymized and the README states the governance before anything is published."),
    },
    taxon: taxon!(Subsystem, Pilot),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("The review records name no external models, and the README says how nerpa is governed."),
);
