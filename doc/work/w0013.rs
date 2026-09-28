use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!(13,
    title: NonEmptyStr::new("A machine lost after it was opened fails per resource, not for the whole read"),
    slice: crate::slice::s0004,
    origin: WorkOrigin::Divergence {
        specification: crate::rfc::rfc_2026_002,
        violated: NonEmptyStr::new("A machine that did not answer is refused per resource, not per run."),
    },
    taxon: taxon!(Subsystem, Core),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("A package or service on a machine lost after it was opened fails that resource and lets the rest of the read finish."),
);
