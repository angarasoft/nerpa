use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-ali-vpc",
    title: NonEmptyStr::new("A declared Alibaba VPC converges"),
    slice: crate::slice::s_alibaba,
    origin: WorkOrigin::Specification(crate::rfc::rfc_2026_002),
    taxon: taxon!(Subsystem, Alibaba),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("A declared alibaba:vpc is created with CreateVpc, found again as the identity decision says, waited for until DescribeVpcs says it is Available, corrected where an attribute is changeable and replaced where it is not, and deleted when its declaration goes; the plan after apply is empty."),
);
