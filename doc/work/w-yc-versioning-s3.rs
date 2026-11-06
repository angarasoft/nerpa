use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-yc-versioning-s3",
    title: NonEmptyStr::new("A declared bucket's versioning is changed in place through the S3 door"),
    slice: crate::slice::s_stage_3_provider,
    origin: WorkOrigin::Specification(crate::rfc::rfc_2026_002),
    taxon: taxon!(Subsystem, Yandex),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("A declared change to versioning is applied in place — enabled and suspended through the S3 door's PUT, the management update carrying the enabled word its validation demands and the S3 door having the final word; a declaration that does not manage versioning on a bucket without it is refused by name, and the word 'disabled' is refused at the declaration; the wire is pinned against exchanges recorded from a real folder, the S3 exchanges among them."),
);
