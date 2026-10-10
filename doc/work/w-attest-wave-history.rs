use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-attest-wave-history",
    title: NonEmptyStr::new("The wave history is attested and closed"),
    slice: crate::slice::s_attest_wave_history,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("dacc 0.6.0's import attests landings from history by name rather than guessing, and the slug rename left the eleven wave works' old trailers unmapped. The mapping is read from the rename commit, and each work is attested to the commit that carried its outcome into the tree."),
    },
    taxon: taxon!(Subsystem, Pilot),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("The eleven remaining wave works are landed from history with evidence, and slices s-wave-0-switch, s-wave-1-decisions, s-wave-2-plan and s-wave-3-proposals are closed."),
);
