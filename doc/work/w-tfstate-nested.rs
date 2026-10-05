use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-tfstate-nested",
    title: NonEmptyStr::new("The import reads the nested attributes Terraform writes, and skips data sources"),
    slice: crate::slice::s_stage_3_cloud,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new(
            "The import parsed attributes as a flatmap, but the terraform.tfstate Terraform writes carries nested objects and lists. It also read data sources as though they were managed. A real state therefore did not import: a data source became a resource, and a nested value such as resources[0].cores could not be reached."
        ),
    },
    taxon: taxon!(Subsystem, Cli),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new(
        "A real terraform.tfstate imports: nested attributes reach their values, a data source is skipped, and yandex_compute_instance, yandex_vpc_network and yandex_vpc_subnet still map to nerpa addresses."
    ),
);
