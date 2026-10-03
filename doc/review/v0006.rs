use dacc_core::{NonEmptyStr, nonempty_str};

dacc_knowledge::review!("v0006",
    title: NonEmptyStr::new("Narrow review of RFC 0001, 2026-09-11"),
    angle: NonEmptyStr::new("Four angles on revision 3 (6f5a6ed) and step 1 only — P and S by reviewer A, B and C by reviewer B; X moved to RFC 0002."),
    findings: nonempty_str![
        "P3-8, P3-13 and P3-17 resolved; P3-1, P3-2, P3-14 and P3-15 partly; P3-16 not.",
    ],
    authors: nonempty_str!["Reviewer A", "Reviewer B"],
    decided_at: dacc_core::date!(2026, 9, 11),
);
