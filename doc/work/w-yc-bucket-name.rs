use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-yc-bucket-name",
    title: NonEmptyStr::new("A bucket's name is refused unless the cloud can hold it"),
    slice: crate::slice::s_stage_3_provider,
    origin: WorkOrigin::Divergence {
        specification: crate::rfc::rfc_2026_002,
        limitation: crate::limitation::l_bucket_name_unchecked,
    },
    taxon: taxon!(Subsystem, Yandex),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("A declared bucket name that is not one the cloud can hold — three to sixty-three characters, lower case, digits and hyphens, starting and ending with a letter or digit — is refused by name at the read and the act, naming the rules it broke, held by a test that was green before the refusal existed because the fake round-tripped whatever it was given."),
);
