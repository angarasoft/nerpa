use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-verifiable-log",
    title: NonEmptyStr::new("The log verifies from the first entry"),
    slice: crate::slice::s_stage_4_access,
    origin: WorkOrigin::Decision(crate::adr::adr_2026_0009),
    taxon: taxon!(Subsystem, Cli),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new(
        "The run journal beside state is an append-only hash chain: every entry carries the entry before it, `nerpa journal verify` walks from the first entry and says so, and an entry rewritten, reordered or removed is found and named rather than read past."
    ),
);
