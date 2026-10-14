use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-yc-schema",
    title: NonEmptyStr::new("The Yandex provider names its resource kinds and their schema"),
    slice: crate::slice::s_stage_3_provider,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("A control-domain provider begins where the os provider did: the kinds it owns and, for each, which attributes are immutable (zone), born (image, user data) and identity (name, id). The distinction is the one a preemptible YC machine makes: a preemption keeps the disk, a recreation does not."),
    },
    taxon: taxon!(Subsystem, Yandex),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("The nerpa-provider-yandex crate exists and names yandex:instance, yandex:disk, yandex:network, yandex:subnet and yandex:security_group, each with its immutable, at-birth and identity attributes, and Provider::fake answers."),
);
