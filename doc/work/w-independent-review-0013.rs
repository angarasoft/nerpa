use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-independent-review-0013",
    title: NonEmptyStr::new("The tree is read by a hand that wrote none of it"),
    slice: crate::slice::s_wave_3_proposals,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("nothing has read the registry's and the prose's claims against the code with a hand that wrote none of it since v0012; RULES.md says promotion wants a reader who did not write the entry, and the drift class TESTING.md once confessed — a document claiming a harness that does not exist — is exactly what only a fresh reader finds"),
    },
    taxon: taxon!(Subsystem, Pilot),
    radius: BlastRadius::Local,
    outcome: NonEmptyStr::new("A review record v0013: every finding names what was seen, where (file:line), why it matters and what to do, ranked blocker to nit, and the stage-4 live-run question is answered with the accesses it needs."),
);
