use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-invariants-second-read",
    title: NonEmptyStr::new("A second hand verifies the thirteen adversarial tests"),
    slice: crate::slice::s_invariants_second_read,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new(
            "W-012: ten invariants were written, attacked and promoted to enforced by the same hand, and RULES.md says promotion wants a reader who did not write the entry."
        ),
    },
    taxon: taxon!(Subsystem, Core),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new(
        "Review v0014 reads the thirteen invariants W-012 names, each adversarial test held against the bypass its entry names; INV-SECRET-003's test is found to miss its bypass, and the missing attempt is written and the entry pointed at it."
    ),
);
