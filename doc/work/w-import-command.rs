use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-import-command",
    title: "A nerpa import command turns a Terraform state into configuration and state",
    slice: crate::slice::s_stage_3_cloud,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new(
            "The import crate maps a state and renders a configuration, but nothing on the command line drives it: a person has to be able to point nerpa at a tfstate and leave with a main.star and a state file, or the import is a library nobody uses."
        ),
    },
    taxon: taxon!(Subsystem, Cli),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new(
        "`nerpa import <tfstate>` writes a main.star that declares the mapped resources and a state file that records them, and prints every secret it found in clear text."
    ),
);
