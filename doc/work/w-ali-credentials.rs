use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-ali-credentials",
    title: NonEmptyStr::new("The Alibaba provider's credential path holds the model"),
    slice: crate::slice::s_alibaba,
    origin: WorkOrigin::Decision(crate::adr::adr_2026_080),
    taxon: taxon!(Subsystem, Alibaba),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("A session comes from an instance RAM role in hardened mode, token first, from an OIDC token exchanged anonymously at STS, or from a session handed over; a pair without a security token is refused by name; a session is refreshed a minute before it expires; its secret and token are wiped when it is replaced and printed by no Debug output."),
);
