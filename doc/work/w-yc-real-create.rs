use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-yc-real-create",
    title: NonEmptyStr::new("The Yandex provider creates and resizes an instance in a real cloud"),
    slice: crate::slice::s_stage_3_provider,
    origin: WorkOrigin::Divergence {
        specification: crate::rfc::rfc_2026_002,
        limitation: crate::limitation::l_yc_cannot_create,
    },
    taxon: taxon!(Subsystem, Yandex),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("The create request carries a boot disk and network interfaces, an operation is polled until it is done, a running instance is stopped, updated and started, the rendered plan shows the stop a change requires, and stop_to_change and monotonic are declared in Yandex::schema."),
);
