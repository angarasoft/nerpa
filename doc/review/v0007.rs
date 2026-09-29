use dacc_core::{NonEmptyStr, nonempty_str};

crate::review!(
    title: NonEmptyStr::new("Fifth review of RFC 0001, 2026-09-11"),
    angle: "P and S, both by reviewer A, on revision 4 (d11060f) and step 1 only.",
    findings: nonempty_str![
        "P4-4, P4-5, P4-9, P4-14 and P4-15 resolved; the rest partly.",
    ],
    authors: nonempty_str!["Reviewer A"],
    decided_at: dacc_core::date!(2026, 9, 11),
);
