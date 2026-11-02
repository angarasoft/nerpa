use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-fleet-closed-document",
    title: NonEmptyStr::new("The fleet document refuses a table it does not know"),
    slice: crate::slice::s_fleet_closed,
    origin: WorkOrigin::Divergence {
        specification: crate::rfc::rfc_2026_004,
        limitation: crate::limitation::l_fleet_skips_unknown_tables,
    },
    taxon: taxon!(Subsystem, Config),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("Fleet::read refuses a top-level table it does not read, naming it and the tables it does; refuses nodes, groups, approval, grants and cloud when they are not tables; and refuses a cloud it has no provider for, naming the clouds it has."),
);
