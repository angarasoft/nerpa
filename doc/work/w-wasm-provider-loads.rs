use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-wasm-provider-loads",
    title: NonEmptyStr::new("A provider built outside this repository loads"),
    slice: crate::slice::s_stage_6_others,
    origin: WorkOrigin::Decision(crate::adr::adr_2026_0004),
    taxon: taxon!(Subsystem, Core),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new(
        "A WASM component built against the WIT interface loads beside the in-tree providers, and its declared kinds appear in the schema a configuration can name."
    ),
);
