use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-control-domain-unreachable",
    title: NonEmptyStr::new("A control-domain resource behind an unreachable machine stays in the plan"),
    slice: crate::slice::s_core_converges,
    origin: WorkOrigin::Divergence {
        specification: crate::rfc::rfc_2026_002,
        limitation: crate::limitation::l_waiting_says_waiting,
    },
    taxon: taxon!(Subsystem, Core),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("A control-domain resource behind an unreachable machine remains in the plan, reported as waiting."),
);
