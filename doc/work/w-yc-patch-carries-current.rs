use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-yc-patch-carries-current",
    title: NonEmptyStr::new("A bucket update carries the object lock and the policy the bucket holds"),
    slice: crate::slice::s_stage_3_provider,
    origin: WorkOrigin::Divergence {
        specification: crate::rfc::rfc_2026_002,
        limitation: crate::limitation::l_patch_resets_unmanaged,
    },
    taxon: taxon!(Subsystem, Yandex),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("The update body carries the object lock and the private-endpoint policy the read carried, not constants — the shapes the validation demands, filled with the world's own words — held by a pinned exchange whose body was refused as a mismatch before the fix; a tag change touches the tags and nothing else, and the read carries both fields back so the patch can."),
);
