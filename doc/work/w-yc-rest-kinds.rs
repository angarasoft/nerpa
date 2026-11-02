use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-yc-rest-kinds",
    title: NonEmptyStr::new("The Yandex provider manages disks, security groups and buckets over REST"),
    slice: crate::slice::s_stage_3_provider,
    origin: WorkOrigin::Specification(crate::rfc::rfc_2026_002),
    taxon: taxon!(Subsystem, Yandex),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("The disk, security_group and bucket kinds converge over REST, with exchanges recorded again from a real folder."),
);
