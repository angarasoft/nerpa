use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-provider-reads-only",
    title: NonEmptyStr::new("A provider reaches a machine only to read"),
    slice: crate::slice::s_stage_2_immutable,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("INV-PROVIDER-002 is claimed but not enforced: the bypass — a provider handed the write side of a change — is described but not yet attacked."),
    },
    taxon: taxon!(Subsystem, Core),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("A provider is handed facts to read and never the write side of a change; the engine sends what a change is made of."),
);
