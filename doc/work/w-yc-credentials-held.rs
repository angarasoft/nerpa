use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-yc-credentials-held",
    title: NonEmptyStr::new("The Yandex provider's credential path holds the model"),
    slice: crate::slice::s_stage_3_provider,
    origin: WorkOrigin::Decision(crate::adr::adr_2026_075),
    taxon: taxon!(Subsystem, Yandex),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("A long-lived credential shape is refused by name, held by a test; the key is zeroized once the JWT is signed; no Debug prints key or token material; and the token is refreshed before expires_at."),
);
