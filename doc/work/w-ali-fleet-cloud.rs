use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-ali-fleet-cloud",
    title: NonEmptyStr::new("The fleet names an Alibaba cloud, and the engine registers its provider"),
    slice: crate::slice::s_alibaba,
    origin: WorkOrigin::Decision(crate::adr::adr_2026_080),
    taxon: taxon!(Subsystem, Alibaba),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("[cloud.alibaba] takes a region and at most one credential source — none, which is the instance role, the OIDC provider, role and token file, or a session; a long-lived pair is refused by name and a session past its expiration is refused; the fleet's Debug output prints no secret; and the engine registers the provider with the credential and the regional endpoints."),
);
