use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-sign-the-plan",
    title: NonEmptyStr::new("One party applies and another signs"),
    slice: crate::slice::s_stage_4_access,
    origin: WorkOrigin::Decision(crate::adr::adr_2026_0009),
    taxon: taxon!(Subsystem, Cli),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new(
        "Where a fleet names who may sign, `apply` proceeds only over a plan a third party has signed: the signature is detached over the plan's number, the approval carries the plan it approves, whoever proposed the plan cannot sign it, and an apply whose plan moved since the signature is refused, naming what moved."
    ),
);
