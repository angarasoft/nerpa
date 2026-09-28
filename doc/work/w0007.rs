use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!(7,
    title: NonEmptyStr::new("The first anchors: a decision names the code it settled"),
    slice: crate::slice::s0002,
    origin: WorkOrigin::Decision(crate::adr::adr_2026_051),
    taxon: taxon!(Subsystem, Gate),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new(
        "FORMAT_VERSION, PROTOCOL_VERSION, the address grammar, the Provider trait and Operation are anchored, and the decisions that settled them reference the anchors."
    ),
);
