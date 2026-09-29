use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-decision-taxonomy",
    title: NonEmptyStr::new("Each decision is classified against the taxonomy"),
    slice: crate::slice::s_wave_1_decisions,
    origin: WorkOrigin::Decision(crate::adr::adr_2026_051),
    taxon: taxon!(Subsystem, Gate),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new(
        "Every converted decision names the subsystem it belongs to, reviewed against the taxonomy; the converter carries the mapping, so a re-generation keeps it."
    ),
);
