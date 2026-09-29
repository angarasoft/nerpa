use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-invariant-type",
    title: NonEmptyStr::new("The invariant type, the conversion, and the anchors"),
    slice: crate::slice::s_wave_3_proposals,
    origin: WorkOrigin::Decision(crate::adr::adr_2026_057),
    taxon: taxon!(Subsystem, Pilot),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new(
        "The local invariant! type exists, the 62 invariants are typed records, the ENFORCES comments are #[doc_anchor]s, cargo xtask invariants reads the registry, and docs/invariants.toml is gone."
    ),
);
