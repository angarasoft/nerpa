use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-yc-key-file",
    title: NonEmptyStr::new("The fleet names a service-account key file, and a long-lived credential in the fleet document is refused by name"),
    slice: crate::slice::s_stage_3_provider,
    origin: WorkOrigin::Decision(crate::adr::adr_2026_079),
    taxon: taxon!(Subsystem, Yandex),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("[cloud.yandex] takes key_file, the path of a service-account key; an API key, a static token, an OAuth token or an inline private key in the fleet document is refused by name, held by a test; a fleet that names both iam_token and key_file is refused; the engine registers the provider with the key file; and the fleet's Debug output prints no token."),
);
