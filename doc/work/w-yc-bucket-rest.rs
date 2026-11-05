use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-yc-bucket-rest",
    title: NonEmptyStr::new("A declared bucket converges over the storage REST API"),
    slice: crate::slice::s_stage_3_provider,
    origin: WorkOrigin::Specification(crate::rfc::rfc_2026_002),
    taxon: taxon!(Subsystem, Yandex),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("A declared yandex:bucket is made with its default storage class, versioning, size cap and tags, found again by name in the folder's list, corrected in place where the storage API allows it and refused where it cannot shrink, and deleted when its declaration goes; the wire is pinned against exchanges recorded from a real folder."),
);
