use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-yandex-provider",
    title: NonEmptyStr::new("The yandex provider manages instance, network and subnet over the Yandex Cloud API"),
    slice: crate::slice::s_stage_3_cloud,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new(
            "Stage 3 has no control-domain provider: a cloud resource cannot be read or changed, and nothing can import a Terraform environment until one exists."
        ),
    },
    taxon: taxon!(Subsystem, Yandex),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new(
        "A yandex provider reads and converges instance, network and subnet against a fake, its tests are recorded HTTP exchanges, and it is wired into the cli beside os."
    ),
);
