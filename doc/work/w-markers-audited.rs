use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-markers-audited",
    title: NonEmptyStr::new("The stale partly-built markers say built"),
    slice: crate::slice::s_architecture_audited,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new(
            "the architecture's markers drift the same way the README did: work lands and the section keeps saying partly built. Two say a gap that is now closed — a grant limits a run, and a provider emits a process — and fourteen name gaps that are really open"
        ),
    },
    taxon: taxon!(Subsystem, Docs),
    radius: BlastRadius::Local,
    outcome: NonEmptyStr::new(
        "The zero-trust section (`0009`) and the wire section (`0033`) say built, because a grant limits a run and a provider emits a process; the other fourteen markers are kept as the gaps they name."
    ),
);
