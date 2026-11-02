use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-ali-recorded-exchanges",
    title: NonEmptyStr::new("The Alibaba provider's wire format is pinned against exchanges recorded from a real account"),
    slice: crate::slice::s_alibaba,
    origin: WorkOrigin::Specification(crate::rfc::rfc_2026_002),
    taxon: taxon!(Subsystem, Alibaba),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("Exchanges for the VPC, vSwitch and instance calls, the metadata service and the OIDC exchange are recorded once from a real Alibaba Cloud account, with no secret, no token and no signature stored, and replayed in tests; the V3 signing is confirmed against the real API. Needs the owner's account: nothing is recorded until the owner provides it."),
);
