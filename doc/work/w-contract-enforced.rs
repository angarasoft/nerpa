use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-contract-enforced",
    title: NonEmptyStr::new("The wire's contract has its executors in the registry"),
    slice: crate::slice::s_stage_4_access,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("`slice close` refuses s-stage-4-access: rfc-2026-003's contract lines have no invariant records — one of them, the write answered with a digest read back, has no record at all although it is built and attacked, and three name guarantees the registry phrases in its own words. A contract nothing points at is a promise nobody checks, which is the drift the obligations mechanism exists to name"),
    },
    taxon: taxon!(Subsystem, Docs),
    radius: BlastRadius::Local,
    outcome: NonEmptyStr::new(
        "Every invariant rfc-2026-003's contract names is a record in the registry with the same words, the write-digest guarantee among them, and s-stage-4-access closes on its outcome."
    ),
);
