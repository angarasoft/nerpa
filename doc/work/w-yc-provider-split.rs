use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-yc-provider-split",
    title: NonEmptyStr::new("The provider's one file becomes four, and the fake tells the truth"),
    slice: crate::slice::s_stage_3_provider,
    origin: WorkOrigin::Specification(crate::rfc::rfc_2026_002),
    taxon: taxon!(Subsystem, Yandex),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("lib.rs is split along the seams it already shows — the wire types and their int64 modules, the attribute mappers and declaration helpers, the bucket's two doors, the in-memory fake — so a reviewer holds one of them at a time, as planner.rs was split before it; the behaviour does not move. The fake refuses an operation on a bucket that is not there instead of answering 200, a read's refusal names the address that caused it rather than the first of the batch, and the fake's publicity says who consumes it and why it is compiled in."),
);
