use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-token-caching",
    title: NonEmptyStr::new("An IAM token is cached until it expires"),
    slice: crate::slice::s_stage_3_cloud,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new(
            "The provider exchanges an OAuth token for an IAM token on every read and act, although an IAM token lives twelve hours. A run reads several batches, so caching the token until its expiry spares the IAM API and the repeat exchange (adr-2026-071)."
        ),
    },
    taxon: taxon!(Subsystem, Yandex),
    radius: BlastRadius::Local,
    outcome: NonEmptyStr::new(
        "A yandex provider resolves its IAM token once and reuses it until it expires; a static token is used as-is, and an expiring one is refreshed only when it is spent."
    ),
);
