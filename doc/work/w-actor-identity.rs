use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-actor-identity",
    title: NonEmptyStr::new("A run carries an actor identity and refuses without one"),
    slice: crate::slice::s_stage_4_access,
    origin: WorkOrigin::Decision(crate::adr::adr_2026_0009),
    taxon: taxon!(Subsystem, Transport),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new(
        "A run refuses to start without an actor identity: the CLI takes `--as`, the engine carries the actor (who, scope, expiry) into the plan and the journal, and the transport proves it still reads no static key — the identity rides ssh's own agent."
    ),
);
