use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-yc-rest-kinds",
    title: NonEmptyStr::new("The Yandex provider manages disks and security groups over REST"),
    slice: crate::slice::s_stage_3_provider,
    origin: WorkOrigin::Specification(crate::rfc::rfc_2026_002),
    taxon: taxon!(Subsystem, Yandex),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("The disk and security_group kinds converge over REST, with exchanges recorded again from a real folder."),
);
