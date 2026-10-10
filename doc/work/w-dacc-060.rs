use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-dacc-060",
    title: NonEmptyStr::new("The registry rides on dacc 0.6.0"),
    slice: crate::slice::s_dacc_060,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("dacc 0.6.0 is published and the tree pins 0.5.2. The minor release adds the norm marker introduces_norm to every decision and specification, so the move is mechanical: each record declares it has no norm, and the classification into norms is a separate work item. It also enforces an executor admission profile for landing, which the upgrade adds."),
    },
    taxon: taxon!(Subsystem, Pilot),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("The workspace compiles against dacc 0.6.0, every decision and specification carries introduces_norm, an executor admission profile exists, and the gate installs dacc-cli at 0.6.0 — the one version line Cargo.toml carries, so the tool and the library cannot drift apart."),
);
