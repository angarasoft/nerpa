use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-yc-provider",
    title: NonEmptyStr::new("A Yandex resource converges end to end"),
    slice: crate::slice::s_stage_3_provider,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("The point of the provider is convergence: declared, created; differing, corrected; gone, removed; and the second plan is empty. An immutable attribute differing means replacement, a birth attribute differing is reported and never acted on."),
    },
    taxon: taxon!(Subsystem, Core),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("A declared yandex:instance and its disk, network, subnet and security group converge: the plan after apply is empty, an immutable change replaces, and a birth change is reported without acting."),
);
