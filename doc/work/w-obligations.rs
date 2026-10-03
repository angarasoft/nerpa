use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-obligations",
    title: NonEmptyStr::new("Every contract has its executors in the registry"),
    slice: crate::slice::s_dacc_050,
    origin: WorkOrigin::Decision(crate::adr::adr_2026_057),
    taxon: taxon!(Subsystem, Pilot),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("The invariant registry compiles against dacc_knowledge's invariant and review types, every invariant names the specification whose contract it holds, the contract check passes for every specification, and s-dacc-050 with s-stage-4-access close on their outcomes."),
);
