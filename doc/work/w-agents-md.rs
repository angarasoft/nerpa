use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-agents-md",
    title: NonEmptyStr::new("AGENTS.md is generated, and the gate says when it is stale"),
    slice: crate::slice::s_dacc_050,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("dacc 0.5.0 emits the agent directives from the registry and checks them, and a generated file nobody re-generates is a generated file that disagrees with its source — the same drift the book's check exists for"),
    },
    taxon: taxon!(Subsystem, Pilot),
    radius: BlastRadius::Local,
    outcome: NonEmptyStr::new("AGENTS.md sits in the tree, written by `cargo dacc emit all` and never by hand, and `cargo xtask ci` refuses a tree where it is stale."),
);
