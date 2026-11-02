use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-yc-wlif",
    title: NonEmptyStr::new("A CI run takes an IAM token for its federated workload identity, with no key"),
    slice: crate::slice::s_stage_3_provider,
    origin: WorkOrigin::Decision(crate::adr::adr_2026_079),
    taxon: taxon!(Subsystem, Yandex),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("The OIDC token of a CI job is exchanged for an IAM token at Yandex IAM's workload identity federation, so the run holds no service-account key. The request and the answer are read from the documented exchange before the code is written, and the exchange is held by a test through Api. In the backlog: nothing is built until it is started."),
);
