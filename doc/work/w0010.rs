use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!(10,
    title: NonEmptyStr::new("Removing a service's declaration stops and disables a service it never declared"),
    slice: crate::slice::s0003,
    origin: WorkOrigin::Divergence {
        specification: crate::rfc::rfc_2026_002,
        violated: NonEmptyStr::new("Removing a resource removes that resource and nothing that was never declared."),
    },
    taxon: taxon!(Subsystem, Os),
    radius: BlastRadius::Local,
    outcome: NonEmptyStr::new("Removing a service declaration no longer stops or disables the service on the machine; the run reports nothing to do for what it never declared."),
);
