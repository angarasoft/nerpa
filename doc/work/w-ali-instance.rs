use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-ali-instance",
    title: NonEmptyStr::new("A declared Alibaba ECS instance converges"),
    slice: crate::slice::s_alibaba,
    origin: WorkOrigin::Specification(crate::rfc::rfc_2026_002),
    taxon: taxon!(Subsystem, Alibaba),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("A declared alibaba:instance is created with RunInstances in its vSwitch and security group under a ClientToken that makes a retry the same request, waited for until it is Running, found again as the identity decision says, replaced where an attribute is immutable, and deleted when its declaration goes; it registers the node it creates; the plan after apply is empty."),
);
