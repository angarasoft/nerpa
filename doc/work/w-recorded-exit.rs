use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-recorded-exit",
    title: NonEmptyStr::new("An imported configuration plans to nothing against a recorded exchange"),
    slice: crate::slice::s_stage_3_cloud,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new(
            "The exit of Stage 3 is that a configuration imported from Terraform plans to nothing. A live cloud needs an IAM token a person holds; a recorded exchange pins the same proof without one, so the exit is a test that runs on every land rather than a one-off command."
        ),
    },
    taxon: taxon!(Subsystem, Yandex),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new(
        "A configuration rendered from an imported Terraform state, read back through a recorded exchange of the same two instances, plans to nothing."
    ),
);
