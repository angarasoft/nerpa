use dacc_core::{NonEmptyStr, nonempty_str};

crate::review!(
    title: NonEmptyStr::new("Wave 4 and the L2 completion, measured after it happened"),
    angle: "A reading of the hypotheses not yet measured — H3 (nobody works around the gate), H5 (done is proven), H6 (radius is labelled honestly), H7 (book-keeping does not grow), and a re-check of H8 (conversion loses nothing) — against the tree as it stands after all eight subsystems reached L2.",
    findings: nonempty_str![
        "H3: every hand-written commit since the switch carries a basis — 104 of 112 commits name a work item, and the eight without one are the auto-generated `slice … closed` journal commits. No forced bypass is on the record: no `--no-verify`, no skipped gate, no refusal pushed over. The one gate step not exercised is `external names`, whose list is empty, so six of seven steps run and refuse.",
        "H5: done is proven — all twenty-five landed work items carry a gate record on the same tree (25 started, 25 gate, 25 landed), and none lands without one. Twelve work items are planned for stages not yet reached, which is work not started rather than work left unproven.",
        "H6: radius is labelled honestly — all thirty-seven work items declare a radius (sixteen crate, twenty-one local) and the registry compiles under the rule that nothing lands above its slice's ceiling, so the ceiling is held by the compiler rather than by memory.",
        "H7: book-keeping does not grow — eighty-four journal records for twenty-five started work items and eight closed slices is a fixed three records per item (started, gate, landed); the overhead is constant per unit of work, not compounding with the number of changes.",
        "H8: conversion loses nothing — v0001 measured the round trip byte-for-byte on forty-eight records, and the immutability rule (a record is superseded, never edited) means that claim has not been invalidated since.",
    ],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 29),
);
