use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-yc-recorded-exchange",
    title: NonEmptyStr::new("Recorded Yandex exchanges replay in tests"),
    slice: crate::slice::s_stage_3_provider,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("The provider's correctness is its mapping of nerpa changes to Yandex calls and back. Testing it against a live cloud is slow and non-deterministic; recording real exchanges and replaying them keeps the test fast and honest."),
    },
    taxon: taxon!(Subsystem, Core),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("Recorded Yandex API exchanges replay in tests, and the provider's read and change paths converge against them without a live cloud."),
);
