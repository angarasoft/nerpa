use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-yc-bucket-lifecycle",
    title: NonEmptyStr::new("A declared bucket moves its versions on a schedule"),
    slice: crate::slice::s_stage_3_provider,
    origin: WorkOrigin::Specification(crate::rfc::rfc_2026_002),
    taxon: taxon!(Subsystem, Yandex),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("A declared `lifecycle` — a list of rules, each with an id, an enabled, a prefix, transitions and expirations for the current and the noncurrent versions — is created with the bucket, carried through every update so a tag change does not wipe the schedule, and cleared by an empty list; the report folds the wire's zero subfields into the words the declaration speaks; the wire is pinned against exchanges recorded from a real folder."),
);
