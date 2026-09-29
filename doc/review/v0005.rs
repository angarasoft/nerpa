use dacc_core::{NonEmptyStr, nonempty_str};

crate::review!(
    title: NonEmptyStr::new("Third review of RFC 0001, 2026-09-11"),
    angle: "The same five angles and models, on revision 2 (d6644b1); P and B ran experiments of their own.",
    findings: nonempty_str![
        "P2-5, P2-11, P2-15, P2-16, P2-17 and P2-18 resolved, the rest partly; S2-2 and S2-10 resolved, the rest partly.",
    ],
    authors: nonempty_str!["Opus", "Sonnet"],
    decided_at: dacc_core::date!(2026, 9, 11),
);
