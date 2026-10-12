use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-anchor-drift",
    title: NonEmptyStr::new("The orphaned anchors are referenced by the invariants they hold"),
    slice: crate::slice::s_migration_tails,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("cargo dacc drift reports 17 anchors no document references — compile-fail and refusal tests marked in the crates during the invariant migration, left unreferenced when the invariant records were written. Each is attached to the invariant whose statement it demonstrates."),
    },
    taxon: taxon!(Subsystem, Pilot),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("cargo dacc drift reports no orphaned anchors: the seventeen anchors are named in the tests of the invariant each one holds."),
);
