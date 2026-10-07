use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-wasm-applies",
    title: NonEmptyStr::new("A WASM provider creates a resource over a real machine"),
    slice: crate::slice::s_stage_6_others,
    origin: WorkOrigin::Decision(crate::adr::adr_2026_0004),
    taxon: taxon!(Subsystem, Core),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new(
        "A WASM provider wired through the run's apply creates a resource on a real machine: its prepare names the file the change settles and the executor creates it, so `nerpa apply` makes what the component declares."
    ),
);
