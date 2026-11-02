use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-ali-vswitch",
    title: NonEmptyStr::new("A declared Alibaba vSwitch converges"),
    slice: crate::slice::s_alibaba,
    origin: WorkOrigin::Specification(crate::rfc::rfc_2026_002),
    taxon: taxon!(Subsystem, Alibaba),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("A declared alibaba:vswitch is created in its VPC and zone, waited for until it is Available, and found again as the identity decision says; its zone, VPC and CIDR block are immutable and a change replaces it; two vSwitches of one VPC are not created at once, because the service refuses that; and the plan after apply is empty."),
);
