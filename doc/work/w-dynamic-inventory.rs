use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-dynamic-inventory",
    title: NonEmptyStr::new("An instance registers the node it creates, without an explicit node()"),
    slice: crate::slice::s_stage_3_cloud,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new(
            "A cloud VM is a node as well as a resource, and a configuration that declares yandex:instance.db then a file on db should not have to say the same thing twice with node(\"db\", provided_by=…). The provider knows an instance names its node; the schema should carry that, and declaring the resource should register the node."
        ),
    },
    taxon: taxon!(Subsystem, Core),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new(
        "A yandex:instance resource registers the node it creates — the graph knows the node is managed by the instance, so a resource on that node is ordered after it, without an explicit node() declaration."
    ),
);
