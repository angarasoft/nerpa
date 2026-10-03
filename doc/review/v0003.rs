use dacc_core::{NonEmptyStr, nonempty_str};

dacc_knowledge::review!("v0003",
    title: NonEmptyStr::new("First review of RFC 0001, 2026-09-11"),
    angle: NonEmptyStr::new("Five angles — protocol and failures, state and invariants, security of self-replacement by reviewer A; performance and the provider contract by reviewer B — on the design as it stood."),
    findings: nonempty_str![
        "Ten defects in the code as it stands were named, each verified by experiment, reading or design reasoning, and fixed: a matching resource never recorded (S-2), state losing resolved and `after` edges (S-2), two addresses sharing one path (S-3), a reaction's debt written too late (S-4), no bound checked before send (B-4), a package premise destabilising another (P-6, not reproduced), a conversation not dropped after a wire error (P-12), a relative executor path accepted (X-3), answer count unchecked (X-8), and 0039 needing a Corrected line (X-10).",
    ],
    authors: nonempty_str!["Reviewer A", "Reviewer B"],
    decided_at: dacc_core::date!(2026, 9, 11),
);
