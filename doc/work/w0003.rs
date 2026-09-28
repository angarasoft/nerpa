use crate::taxonomy::Subsystem;
use slipway_core::{taxon, BlastRadius, NonEmptyStr};
use slipway_work::WorkOrigin;

slipway_work::work!(3,
    title: NonEmptyStr::new("Each decision is classified against the taxonomy"),
    slice: crate::slice::s0002,
    origin: WorkOrigin::Decision(crate::adr::adr_2026_051),
    taxon: taxon!(Subsystem, Gate),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new(
        "Every converted decision names the subsystem it belongs to, reviewed against the taxonomy; the converter carries the mapping, so a re-generation keeps it."
    ),
);
