use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!(9,
    title: NonEmptyStr::new("A service declared without enabled or running is not disabled or stopped when it is updated"),
    slice: crate::slice::s0003,
    origin: WorkOrigin::Divergence {
        specification: crate::rfc::rfc_2026_002,
        violated: NonEmptyStr::new("An attribute a resource was born with is reported, never turned into work."),
    },
    taxon: taxon!(Subsystem, Os),
    radius: BlastRadius::Local,
    outcome: NonEmptyStr::new("A service declared without enabled or running leaves both unmanaged; the run after a change is empty, and the service is neither disabled nor stopped."),
);
