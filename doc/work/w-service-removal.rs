use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-service-removal",
    title: NonEmptyStr::new("Removing a service's declaration stops and disables a service it never declared"),
    slice: crate::slice::s_os_converges,
    origin: WorkOrigin::Divergence {
        specification: crate::rfc::rfc_2026_002,
        limitation: crate::limitation::l_removal_removes_the_declared,
    },
    taxon: taxon!(Subsystem, Os),
    radius: BlastRadius::Local,
    outcome: NonEmptyStr::new("Removing a service declaration no longer stops or disables the service on the machine; the run reports nothing to do for what it never declared."),
);
