use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-dacc-050",
    title: NonEmptyStr::new("The registry moves onto dacc 0.5.0"),
    slice: crate::slice::s_dacc_050,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("dacc 0.5.0 is published: it brings the obligations mechanism — a contract's executors — the AGENTS.md emitter and the new journal shape, and the tree pins 0.3.0, whose record model can carry none of it. W-030 stands between s-stage-4-access and a closed slice for exactly this reason."),
    },
    taxon: taxon!(Subsystem, Pilot),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("The registry compiles against dacc 0.5.0, its records declare what 0.5.0 requires, and the gate installs and runs dacc-cli 0.5.0."),
);
