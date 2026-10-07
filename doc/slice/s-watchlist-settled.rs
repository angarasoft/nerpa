use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-watchlist-settled",
    title: NonEmptyStr::new("The watchlist's remaining maintenance is settled"),
    thrust: crate::thrust::t_hygiene,
    outcome: NonEmptyStr::new(
        "The three stale dacc-internal entries close as resolved, and a gate refuses a path that confuses the compiled registry `doc/` with the prose `docs/` (W-028)."
    ),
    specification: crate::rfc::rfc_2026_001,
    max_radius: BlastRadius::Local,
);
