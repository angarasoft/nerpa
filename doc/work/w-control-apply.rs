use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-control-apply",
    title: NonEmptyStr::new("The engine applies control-domain changes through a provider's act"),
    slice: crate::slice::s_stage_3_cloud,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new(
            "A control resource can be read but not applied: the apply path is machine-shaped (Operation, Reach, executor) and a control resource's reach is Nowhere, which refuses every ask. p0004 designs the act() method; this work lands it."
        ),
    },
    taxon: taxon!(Subsystem, Core),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new(
        "A control-domain change applies through Provider::act — the provider performs its own call, the mark is written before and the conclusion after, with no Reach in between — and a fake control provider converges in the suite."
    ),
);
