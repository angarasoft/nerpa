use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-plan-idempotent",
    title: NonEmptyStr::new("Applying a plan and planning again produces an empty plan"),
    slice: crate::slice::s_stage_2_immutable,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("INV-PLAN-003 is claimed but not enforced: the bypass — a change that applies but is not recorded, so the next plan repeats it — is described but not yet attacked."),
    },
    taxon: taxon!(Subsystem, Core),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("Applying a plan and planning again produces an empty plan."),
);
