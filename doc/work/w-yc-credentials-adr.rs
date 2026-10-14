use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-yc-credentials-adr",
    title: NonEmptyStr::new("An ADR names the Yandex credential model honestly"),
    slice: crate::slice::s_stage_3_provider,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("The landed w-yc-api claims a long-lived key is never a mode, but the flow begins with one: the service-account key is the long-lived input the token is minted from, and it cannot be avoided. The claim lives in a work's justification string, held by no code, which ADR-069 forbids; i0030's rationale now says no code path reads a long-lived key, which the client falsifies. A decision names the model as it is, with an executor."),
    },
    taxon: taxon!(Subsystem, Yandex),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("A decision records that the service-account key is the long-lived input and the carried credential is the short-lived IAM token refreshed before expiry, with a long-lived credential refused by default and an executor named; i0030's rationale is scoped to actor identity."),
);
