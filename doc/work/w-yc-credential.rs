use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-yc-credential",
    title: NonEmptyStr::new("The yandex provider derives its IAM token from the yc profile"),
    slice: crate::slice::s_stage_3_cloud,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new(
            "A run needs a short-lived IAM token, and today a person fetches it through a wrapper script — `yc iam create-token` — before every run. The machine already holds the OAuth token in the yc config, so the provider should derive the IAM token itself, the way the script did."
        ),
    },
    taxon: taxon!(Subsystem, Yandex),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new(
        "The yandex provider derives its IAM token from the yc profile (adr-2026-071): `iam_token` is optional in the fleet, and a full YAML parser reads the profile."
    ),
);
