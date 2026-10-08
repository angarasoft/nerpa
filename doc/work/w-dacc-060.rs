use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-dacc-060",
    title: NonEmptyStr::new("The registry rides on dacc 0.6.0"),
    slice: crate::slice::s_dacc_060,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("dacc 0.6.0 is published and the tree pins 0.5.2. A minor release of the methodology the registry runs on is upkeep rather than a decision: the same records carry it, and what moves is the version the gate installs beside them."),
    },
    taxon: taxon!(Subsystem, Pilot),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("The workspace compiles against dacc 0.6.0, and the gate installs dacc-cli at 0.6.0 — the one version line Cargo.toml carries, so the tool and the library cannot drift apart."),
);
