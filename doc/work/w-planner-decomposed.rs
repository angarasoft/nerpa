use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-planner-decomposed",
    title: NonEmptyStr::new("planner.rs is split into four modules without changing behaviour"),
    slice: crate::slice::s_planner_decomposed,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new(
            "W-033: the single-pass planner, the removal ordering and the per-resource change decision are concentrated in one 1686-line file — the node where a subtle ordering mistake destroys infrastructure."
        ),
    },
    taxon: taxon!(Subsystem, Core),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new(
        "planner.rs is decomposed into walk, removals, change and naming modules; Planning::new and validate_attribute are extracted; every doc_anchor id is preserved and the full test suite stays green."
    ),
);
