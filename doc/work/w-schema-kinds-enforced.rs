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
    outcome: NonEmptyStr::new("The planner refuses a shrink of a monotonic attribute by name, and a stop-to-change delta is marked rather than refused: the delta's forces_replacement bool becomes a DeltaKind — InPlace, StopThenChange or Replace — carried in the canonical encoding, so the plan shows the stop it requires and the provider sequences stop, change, start. Held by tests."),
);
