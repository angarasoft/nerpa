use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-rustls-advisories",
    title: NonEmptyStr::new("The lock leaves the rustls advisories and the yanked clipboard-win"),
    slice: crate::slice::s_develop_rejoined,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("the advisories RUSTSEC-2026-0346, RUSTSEC-2026-0347 and RUSTSEC-2026-0348 on rustls 0.23.45 and the yanked clipboard-win 5.4.1 turn cargo deny red, so the gate cannot pass on any tree that carries this lock; updating the two crates is upkeep, not a decision"),
    },
    taxon: taxon!(Subsystem, Gate),
    radius: BlastRadius::Local,
    outcome: NonEmptyStr::new("Cargo.lock no longer names rustls 0.23.45 or clipboard-win 5.4.1, and cargo deny check passes."),
);
