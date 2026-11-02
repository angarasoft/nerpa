use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-adr-ali-identity",
    title: NonEmptyStr::new("An ADR names how an Alibaba resource is found again"),
    slice: crate::slice::s_alibaba,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("The documentation of Alibaba's VPC and ECS list calls does not say that a name is unique, and a list call filters by name, by tag or by id. How a declared resource is found again after it was made — and how a create that timed out is not made twice, with a ClientToken of at most 64 ASCII characters — has to be decided before the first kind writes it down."),
    },
    taxon: taxon!(Subsystem, Alibaba),
    radius: BlastRadius::Local,
    outcome: NonEmptyStr::new("A decision names how a VPC, a vSwitch and an instance are found again, what a create sends as ClientToken so that a retry is the same request and a changed declaration is a new one, and how an imported resource is found by its id."),
);
