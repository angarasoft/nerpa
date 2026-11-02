use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-adr-ali-credentials",
    title: NonEmptyStr::new("An ADR names how the Alibaba provider proves who it is"),
    slice: crate::slice::s_alibaba,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("Alibaba Cloud signs every request with an AccessKey pair, so the secret that signs lives for the whole session and not for one signature as a Yandex service-account key does, and adr-2026-079 does not say what that changes. A decision names the sources a session may come from, refuses the long-lived pair, and states what is wiped, before the provider's code depends on it."),
    },
    taxon: taxon!(Subsystem, Alibaba),
    radius: BlastRadius::Local,
    outcome: NonEmptyStr::new("A decision names the carried credential as a short-lived session, the three sources of version one — an instance RAM role through the metadata service in hardened mode, an OIDC token exchanged at STS, a session handed over in the fleet — the refusal of a long-lived pair, the V3 signing, and what is wiped."),
);
