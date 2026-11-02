use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-yc-key-copies-wiped",
    title: NonEmptyStr::new("No copy of the Yandex service-account key outlives its signature"),
    slice: crate::slice::s_stage_3_provider,
    origin: WorkOrigin::Divergence {
        specification: crate::rfc::rfc_2026_002,
        limitation: crate::limitation::l_yc_key_copies_unwiped,
    },
    taxon: taxon!(Subsystem, Yandex),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("The signer behind the Signer seam is one that wipes the key it parsed, or one outside the process, and a test shows that no copy of the key stays reachable through the signer after the signature. In the backlog: nothing is built until it is started."),
);
