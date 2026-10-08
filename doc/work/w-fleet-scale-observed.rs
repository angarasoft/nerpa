use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-fleet-scale-observed",
    title: NonEmptyStr::new("The watchlist records the missing rollout axis and failure policy"),
    slice: crate::slice::s_fleet_scale,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new(
            "the product goal is parallel management of a thousand machines, and applying to all of them at once with no canary and no failure policy is the thing nobody does; the gaps are recorded before the fleet grows at stage 3"
        ),
    },
    taxon: taxon!(Subsystem, Core),
    radius: BlastRadius::Local,
    outcome: NonEmptyStr::new(
        "W-034 names the missing rollout axis and failure policy, the group concept not reaching the plan, and the descriptive-only BlastRadius."
    ),
);
