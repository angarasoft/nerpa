use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-introduces-norm-policy",
    title: NonEmptyStr::new("The norm marker is marked on demand, and the policy is recorded"),
    slice: crate::slice::s_migration_tails,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("dacc 0.6.0 added introduces_norm, and the migration set it false on all 72 records mechanically. The user decides the marker is set when a norm is introduced, not backfilled, and the policy is recorded as a decision so agents read it instead of guessing."),
    },
    taxon: taxon!(Subsystem, Pilot),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("A decision records that introduces_norm is marked when a norm is introduced, not backfilled; the 72 records migrated onto 0.6.0 stay false until touched."),
);
