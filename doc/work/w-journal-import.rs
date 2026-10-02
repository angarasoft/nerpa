use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-journal-import",
    title: NonEmptyStr::new("The wave journal is imported where history carries a basis"),
    slice: crate::slice::s_dacc_050,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("W-027 leaves fifteen works folding as planned although their artefacts are in the tree, and dacc 0.5.0's import reads both the new trailer and the old Slipway-Work one — the first tool that could see the wave history. Whether it restores it without inventing landings is W-029's question, and the answer decides whether the import runs or is refused"),
    },
    taxon: taxon!(Subsystem, Pilot),
    radius: BlastRadius::Local,
    outcome: NonEmptyStr::new("The import runs at 0.5.0 where history carries a work's basis and is held where it would invent a landing: what it restored, what it could not and why are recorded in W-027 and W-029."),
);
