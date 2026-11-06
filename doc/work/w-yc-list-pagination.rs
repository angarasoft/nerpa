use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-yc-list-pagination",
    title: NonEmptyStr::new("The three lists read every page of a folder"),
    slice: crate::slice::s_stage_3_provider,
    origin: WorkOrigin::Divergence {
        specification: crate::rfc::rfc_2026_002,
        limitation: crate::limitation::l_lists_read_one_page,
    },
    taxon: taxon!(Subsystem, Yandex),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("The instance, network and subnet lists follow the page token to the last page through one shared walk, held by a pinned exchange whose resource lives on the second page and read as absent before the fix; a folder larger than one page is read whole, and the plan it feeds no longer proposes a duplicate of what the first page could not see."),
);
