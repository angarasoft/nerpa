use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-yc-api",
    title: NonEmptyStr::new("The Yandex Cloud API is reached with a short-lived token"),
    slice: crate::slice::s_stage_3_provider,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("A cloud provider reaches the control domain by API, not by a machine. The maximum-security shape is a short-lived IAM token exchanged from a service account; a long-lived key or API key is the downgrade and is refused by default."),
    },
    taxon: taxon!(Subsystem, Core),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("The compute and vpc resources are read and changed through the Yandex gRPC API, authenticated by a short-lived token; no long-lived key is a mode."),
);
