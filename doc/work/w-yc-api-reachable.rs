use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-yc-api-reachable",
    title: NonEmptyStr::new("The Yandex client reaches the API it was landed for"),
    slice: crate::slice::s_stage_3_provider,
    origin: WorkOrigin::Divergence {
        specification: crate::rfc::rfc_2026_002,
        limitation: crate::limitation::l_yc_api_unreachable,
    },
    taxon: taxon!(Subsystem, Yandex),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("The client signs a PS256 JWT with ring and exchanges it for an IAM token over the same gRPC channel it reads resources through, refreshed before expires_at; the channel speaks TLS with timeouts; the key is never printed and is zeroized after signing; the vendored protos carry provenance and the build needs no system protoc. rsa and ureq leave the tree, and the RUSTSEC-2023-0071 ignore is lifted."),
);
