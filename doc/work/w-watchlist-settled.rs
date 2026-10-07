use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-watchlist-settled",
    title: NonEmptyStr::new("A gate refuses a doc/ vs docs/ path confusion, and the stale dacc entries close"),
    slice: crate::slice::s_watchlist_settled,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new(
            "W-028: doc/ and docs/ are one letter apart and nothing refuses the mistake; W-029, W-031 and W-032 describe dacc internals that are already resolved at the root, so they close as observations whose answer arrived."
        ),
    },
    taxon: taxon!(Subsystem, Gate),
    radius: BlastRadius::Local,
    outcome: NonEmptyStr::new(
        "cargo xtask docs refuses a prose reference that names a doc/ path whose docs/ sibling exists, or the reverse; W-029, W-031 and W-032 leave the watchlist as resolved dacc internals."
    ),
);
