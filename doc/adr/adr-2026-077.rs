use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "Buckets are managed through the Yandex gRPC service, not the S3 REST API",
    status: DocStatus::SupersededBy(crate::adr::adr_2026_078),
    channel: crate::taxonomy::channel::Public,
    subsystems: &[taxon!(Subsystem, Yandex)],
    context: r"
        A bucket kind joins the provider's stage (w-yc-bucket), and Object
        Storage answers through two doors: the S3-compatible REST API on
        storage.yandexcloud.net, authenticated with AWS-style SigV4, and the
        Yandex gRPC BucketService (yandex.cloud.storage.v1) on
        storage.api.cloud.yandex.net, authenticated like every other service
        through the IAM token. A probe of both shows both alive.

        The provider already speaks the second door's language: tonic, the
        token exchange, the native root store, and the recorded-exchange
        tunnel all exist. The first door would need a second HTTP stack and a
        hand-rolled SigV4 signer, which is exactly the kind of second
        mechanism the tree has been consolidating away from.
    ",
    decision: r"
        A declared yandex:bucket is managed through the Yandex gRPC
        BucketService on storage.api.cloud.yandex.net: listed per folder,
        made, and deleted, read back by name. The recorded-exchange machinery
        covers the bucket path as it covers the others — the same tunnel
        fans the bucket calls out, and the same replay answers them, so a
        bucket's exchanges record and replay without a live cloud.

        The S3 REST API is not used: one transport serves the whole stage,
        and SigV4 is not brought into the tree.
    ",
    trade_offs: &[
        "The gRPC service is Yandex's own, so what the provider proves is the Yandex mapping, not S3 compatibility; the S3 door is the one other tools speak, and a provider that managed buckets through S3 would prove more to a world of aws cli users. Accepted: the stage converges one cloud first, and the S3 door stays open for a later decision if buckets ever need object-level operations.",
        "Bucket names are globally unique in Object Storage, so an existence check cannot be by name alone in an empty folder; the read is the list of the folder's buckets, which the provider already does for every other kind.",
        "The recorded-exchange machinery gains a bucket path — another service the tunnel serves and the replay answers — and the cost is a second live-cloud fixture to keep honest: a bucket's exchanges record against a real folder and re-take when the mapping changes, one ignored-test run away.",
    ],
    constraints: &[
        "Bucket management goes through yandex.cloud.storage.v1 over the same TLS posture as every other service (ADR-076): native roots, ring provider.",
        "No SigV4 code enters the tree.",
        "The recorded-exchange machinery covers the bucket path: a recording replays in tests without a live cloud.",
    ],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 10, 25),
    breaking: Breaking::No,
    introduces_norm: false,
    code_refs: &[],
    related_rfcs: &[crate::rfc::rfc_2026_002],
);
