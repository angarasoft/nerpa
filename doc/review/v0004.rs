use dacc_core::{NonEmptyStr, nonempty_str};

dacc_knowledge::review!("v0004",
    title: NonEmptyStr::new("Second review of RFC 0001, 2026-09-11"),
    angle: NonEmptyStr::new("The same five angles and models, on revision 1 (a59c0b0)."),
    findings: nonempty_str![
        "The first round's findings were carried into revision 2: P-1, P-2, P-3, P-4 (superseded), P-8, P-12 and P-14 resolved, P-5, P-6, P-7, P-9, P-10, P-11 and P-13 partly; S-2, S-4, S-6 and S-9 resolved, S-1, S-3, S-5, S-7, S-8, S-10 and S-11 partly.",
    ],
    authors: nonempty_str!["Reviewer A", "Reviewer B"],
    decided_at: dacc_core::date!(2026, 9, 11),
);
