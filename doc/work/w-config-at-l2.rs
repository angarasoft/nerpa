use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-config-at-l2",
    title: NonEmptyStr::new("The config is declared at L2"),
    slice: crate::slice::s_subsystems_to_l2,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("the configuration language had no baseline specification, and five of its invariants were not yet enforced, so its work could not land with a gate proof"),
    },
    taxon: taxon!(Subsystem, Config),
    radius: BlastRadius::Local,
    outcome: NonEmptyStr::new("A work item whose radius reaches Config lands only with a gate proof; the baseline names Config's invariants, and the independence a radius requires is a review record."),
);
