use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-yc-empty-lists",
    title: NonEmptyStr::new("An empty folder reads as no instances, networks or subnets"),
    slice: crate::slice::s_stage_3_provider,
    origin: WorkOrigin::Divergence {
        specification: crate::rfc::rfc_2026_002,
        limitation: crate::limitation::l_empty_folder_reads_as_refusal,
    },
    taxon: taxon!(Subsystem, Yandex),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("The instance, network and subnet lists read a folder the API answers with `{}` as a world that holds none of them, held by a pinned exchange each, and a fresh folder plans a create rather than refusing its own observation."),
);
