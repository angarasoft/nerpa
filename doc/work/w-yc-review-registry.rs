use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-yc-review-registry",
    title: NonEmptyStr::new("The review's registry findings land on the Yandex works"),
    slice: crate::slice::s_stage_3_provider,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("The independent review of the landed Yandex works found the registry disagrees with itself: the four w-yc-* works are declared Core while their commits carry the yandex scope, and cargo dacc conformance shows no yandex row. The works own the nerpa-provider-yandex crate, so they carry the subsystem that owns it."),
    },
    taxon: taxon!(Subsystem, Yandex),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("The four w-yc-* works carry the Yandex subsystem, and cargo dacc conformance shows the yandex row."),
);
