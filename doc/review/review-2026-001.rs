use slipway_core::{NonEmptyStr, nonempty_str};

crate::review!(
    title: NonEmptyStr::new("The conversion, read after it happened"),
    angle: "A reading of the converted records against what the round trip checked, looking for what it did not check.",
    findings: nonempty_str![
        "The round trip held byte for byte on all forty-eight records, and the five with extra sections keep those sections in the context field — none loses its reasoning, and each is decided one by one.",
        "The classification is a first pass against the taxonomy; it is a review of a mapping, not a claim that the mapping is final.",
    ],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: slipway_core::date!(2026, 9, 28),
);
