use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-stale-attribute-reference",
    title: NonEmptyStr::new("A reference to an attribute changed in the same plan resolves to the new value"),
    slice: crate::slice::s_core_converges,
    origin: WorkOrigin::Divergence {
        specification: crate::rfc::rfc_2026_002,
        limitation: crate::limitation::l_replanning_is_empty,
    },
    taxon: taxon!(Subsystem, Core),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("A reference to an attribute changed in the same plan resolves to the new value, so the run after applying is empty."),
);
