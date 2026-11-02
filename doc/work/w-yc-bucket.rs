use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-yc-bucket",
    title: NonEmptyStr::new("Declared buckets converge against Object Storage"),
    slice: crate::slice::s_stage_3_provider,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("The cloud's vpc and compute quotas are spent after the datacenter attack, and Object Storage answers through its own API without them; a bucket is the storage resource the stage can still converge, and its exchanges record and replay through the machinery w-yc-recorded-exchange built."),
    },
    taxon: taxon!(Subsystem, Yandex),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("A declared yandex:bucket converges: it is made and removed against Object Storage, and recorded bucket exchanges replay in tests without a live cloud."),
);
