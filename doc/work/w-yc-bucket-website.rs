use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-yc-bucket-website",
    title: NonEmptyStr::new("A declared bucket serves a static site"),
    slice: crate::slice::s_stage_3_provider,
    origin: WorkOrigin::Specification(crate::rfc::rfc_2026_002),
    taxon: taxon!(Subsystem, Yandex),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("A declared website — an index page and an error page — and the anonymous flags that open a bucket to the public are created, carried and corrected through the management door; every update carries the site the read carried, so a change to a tag no longer wipes it, held by a pinned exchange whose body was a wipe before; the wire is pinned against exchanges recorded from a real folder."),
);
