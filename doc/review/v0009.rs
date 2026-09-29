use dacc_core::{NonEmptyStr, nonempty_str};

crate::review!(
    title: NonEmptyStr::new("Narrow round of RFC 0001, 2026-09-17"),
    angle: "P and S, both by reviewer A, on revision 6 (6aa163b), over §5.5 to §5.8 and §5.13 and the code built since the fifth review.",
    findings: nonempty_str![
        "P6-6, P6-8, P6-10, P6-11, P6-12, P6-14, S6-5, S6-6, S6-9, S6-12, S6-13 and S6-14 resolved; the rest partly. Corrected while building, and the built state logged.",
    ],
    authors: nonempty_str!["Reviewer A"],
    decided_at: dacc_core::date!(2026, 9, 17),
);
