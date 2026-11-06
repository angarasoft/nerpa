use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-yc-put-required",
    title: NonEmptyStr::new("The PUT verb is required of every wire, and the JSON verbs label one Content-Type"),
    slice: crate::slice::s_stage_3_provider,
    origin: WorkOrigin::Divergence {
        specification: crate::rfc::rfc_2026_002,
        limitation: crate::limitation::l_put_optional,
    },
    taxon: taxon!(Subsystem, Yandex),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("Api::put is a required method: a wire that cannot write the versioning document does not compile, and the refusing default leaves the trait. The JSON verbs set the Content-Type rather than appending a second one where the caller named it. A wire that cannot PUT is refused by the compiler, not by the live cloud."),
);
