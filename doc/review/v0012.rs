use dacc_core::{NonEmptyStr, nonempty_str};

dacc_knowledge::review!("v0012",
    title: NonEmptyStr::new("The verdict, read independently against the criteria written before the start"),
    angle: NonEmptyStr::new("A reading of the nine hypotheses against the records that measured them and the tree as it stands, checking each result against its measure and its stop threshold without having written any other pilot record."),
    findings: nonempty_str![
        "H1 holds — compiled references catch what Markdown checks miss. v0010 found five semantic disagreements the build could not catch while the compiled references stayed green, so the compiled form catches renames and deletions and review catches the semantic mapping.",
        "H2 holds — writing a record costs little. v0002 measured the eighteen wave-2 records in minutes each, compiling on the first build that named the package, neither over the thresholds that would narrow the pilot.",
        "H3 holds — nobody works around the gate. v0011 counted 104 of 112 post-switch commits carrying a basis, the eight without one being the auto-generated slice-close journal commits, and no --no-verify or skipped gate on the record; the one step not exercised is external names, whose list is empty, so six of seven steps run and refuse.",
        "H4 holds — origins stay meaningful. v0002 found the two product slices carry no Toil, a routine share of zero, below the twenty percent that would stop the pilot.",
        "H5 holds — done is proven. v0011 counted every landed work item carrying a gate record on the same tree, and none lands without one.",
        "H6 holds — radius is labelled honestly. v0011 counted thirty-seven work items each declaring a radius, and the ceiling is held by the compiler rather than by memory.",
        "H7 holds — book-keeping does not grow. v0011 counted a fixed three records per work item, so the overhead is constant per unit of work rather than compounding with the number of changes.",
        "H8 holds — conversion loses nothing. v0001 measured the round trip byte-for-byte on forty-eight records, and the immutability rule means that claim has not been invalidated since.",
        "H9 holds — the local types earn their place. v0010 found the local invariant, review and proposal types forced the statuses and the anchor references, and they are not offered upstream.",
        "Verdict: continue — all nine hypotheses hold against the criteria written before the start. The tree has moved past v0011, which does not weaken the measurements; the open items — external names not exercised and seven of sixty-two invariants not yet Enforced — are honest gaps named in SECURITY.md, not failures of a hypothesis.",
    ],
    authors: nonempty_str!["Reviewer C"],
    decided_at: dacc_core::date!(2026, 9, 29),
);
