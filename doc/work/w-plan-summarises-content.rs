use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-plan-summarises-content",
    title: NonEmptyStr::new("A plan cuts large content to a byte count"),
    slice: crate::slice::s_plan_summarises_content,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new(
            "W-006: a plan that prints a megabyte of certificate is a plan nobody reads; a text value past a bound is cut to a prefix and a byte count."
        ),
    },
    taxon: taxon!(Subsystem, Cli),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new(
        "The plan's rendering summarises a text value past a bound as a prefix and a byte count, and clips each printed line the same way, so the plan stays readable regardless of how large a file's content is."
    ),
);
