use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-tfstate-import",
    title: NonEmptyStr::new("A Terraform state imports into nerpa, naming every secret it held"),
    slice: crate::slice::s_stage_3_cloud,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new(
            "Stage 3's way in from Terraform: an existing environment is read out of its state rather than recreated, and the import has to name the secrets the old state held in clear text — the one thing Terraform's state is known to leak."
        ),
    },
    taxon: taxon!(Subsystem, Cli),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new(
        "A terraform.tfstate reads: yandex_compute_instance, yandex_vpc_network and yandex_vpc_subnet become nerpa addresses with their attributes, the import names every secret it found in clear text, and the generated configuration declares the same resources."
    ),
);
