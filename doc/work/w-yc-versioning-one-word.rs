use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-yc-versioning-one-word",
    title: NonEmptyStr::new("The versioning vocabulary is one type, not four maps with a silent fallback"),
    slice: crate::slice::s_stage_3_provider,
    origin: WorkOrigin::Specification(crate::rfc::rfc_2026_002),
    taxon: taxon!(Subsystem, Yandex),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("The declared words live in one enum — Enabled and Suspended — parsed once and refused everywhere else, with the management spelling and the S3 spelling as methods of it; the four string maps and their fall-through arms are gone, so a word that forgets validation cannot compile into a wrong silence. The read maps the wire's words back into the same type, disabled among them, where it belongs: a reported state, not a declared one."),
);
