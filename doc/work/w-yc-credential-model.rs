use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-yc-credential-model",
    title: NonEmptyStr::new("The Yandex provider holds the credential model adr-2026-075 asks for"),
    slice: crate::slice::s_stage_3_provider,
    origin: WorkOrigin::Divergence {
        specification: crate::rfc::rfc_2026_002,
        limitation: crate::limitation::l_yc_credential_model_absent,
    },
    taxon: taxon!(Subsystem, Yandex),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("A service-account key is exchanged for an IAM token as a PS256 JWT over REST through Api; a long-lived credential shape is refused by name and the refusal is held by a test; no Debug output prints key or token material. iam_token stays as adr-2026-071 keeps it for CI: an IAM token is short-lived and is not the static token adr-2026-075 refuses."),
);
