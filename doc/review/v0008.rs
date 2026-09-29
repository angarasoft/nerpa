use dacc_core::{NonEmptyStr, nonempty_str};

crate::review!(
    title: NonEmptyStr::new("Sixth review of RFC 0001, 2026-09-17"),
    angle: "P and S, both by Opus, on revision 5 (28ab691) and step 1 only.",
    findings: nonempty_str![
        "P5-1, P5-4, P5-6, P5-8, P5-9, P5-14, P5-15 and P5-16 resolved, the rest partly; S5-18 resolved.",
    ],
    authors: nonempty_str!["Opus"],
    decided_at: dacc_core::date!(2026, 9, 17),
);
