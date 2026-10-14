use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-schema-kinds-enforced",
    title: NonEmptyStr::new("The planner holds the monotonic and stop-to-change kinds"),
    slice: crate::slice::s_stage_3_provider,
    origin: WorkOrigin::Divergence {
        specification: crate::rfc::rfc_2026_002,
        limitation: crate::limitation::l_schema_kinds_unenforced,
    },
    taxon: taxon!(Subsystem, Core),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("The planner refuses a shrink of a monotonic attribute and marks a stop-to-change attribute for the stopped sequence, held by tests; the Schema's declares are read on the planning path, not only by the Schema's own tests."),
);
