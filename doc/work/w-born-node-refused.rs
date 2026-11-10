use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-born-node-refused",
    title: NonEmptyStr::new("A node a cloud resource creates is refused unless the fleet routes it"),
    slice: crate::slice::s_born_node_refused,
    origin: WorkOrigin::Divergence {
        specification: crate::rfc::rfc_2026_002,
        limitation: crate::limitation::l_born_node_reached_here,
    },
    taxon: taxon!(Subsystem, Cli),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("A node whose provenance is managed and which the fleet gives no route is refused by name, with the resource that creates it, through the per-machine refusal; a node that is not managed still routes here."),
);
