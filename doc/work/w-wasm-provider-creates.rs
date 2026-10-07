use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-wasm-provider-creates",
    title: NonEmptyStr::new("A provider built outside this repository creates a resource"),
    slice: crate::slice::s_stage_6_others,
    origin: WorkOrigin::Decision(crate::adr::adr_2026_0004),
    taxon: taxon!(Subsystem, Core),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new(
        "A WASM component implementing the WIT world's read, prepare and conclude plans and applies a resource over the opened machines, so `nerpa apply` makes what the component declares."
    ),
);
