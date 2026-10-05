use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-instance-network",
    title: NonEmptyStr::new("An instance carries its subnet and boot-disk image, and the import reads both"),
    slice: crate::slice::s_stage_3_cloud,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new(
            "The import mapped an instance's cores and memory but dropped what connects it and what it was born from: its subnet and its boot-disk image. A subnet is immutable — a NIC cannot move — and the image is a birth attribute, reported rather than corrected. Both are in the state, and both should come across."
        ),
    },
    taxon: taxon!(Subsystem, Yandex),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new(
        "A yandex:instance carries its subnet_id (immutable) and boot_disk_image_id (at birth), the provider reads the subnet back from the network interface, and the import reads both from a Terraform state."
    ),
);
