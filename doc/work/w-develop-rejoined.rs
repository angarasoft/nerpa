use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-develop-rejoined",
    title: NonEmptyStr::new("The develop line is rejoined into main with its landing proofs intact"),
    slice: crate::slice::s_develop_rejoined,
    origin: WorkOrigin::Decision(crate::adr::adr_2026_078),
    taxon: taxon!(Subsystem, Pilot),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("The develop line is merged into main as a two-parent commit; the schema kinds the planner enforces, the registry records and the journal events of both lines are joined, every landing proof still names a commit that is an ancestor of the merge, and the Yandex provider stays on the main line's REST crate."),
);
