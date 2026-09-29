use dacc_core::{NonEmptyStr, nonempty_str};

crate::review!(
    title: NonEmptyStr::new("Wave 3, measured after it happened"),
    angle: "A reading of the converted invariants and proposals against the RFCs, decisions and specifications they came from, measuring H1 (compiled references catch what Markdown checks miss) and H9 (the local types earn their place), and looking for what the compiled references could not check.",
    findings: nonempty_str![
        "`p0001.decided` holds all thirteen decisions the RFC `Touches`, but the field is meant to be the decisions it settled. Section 3 `What is decided` separates `Decided already` (eleven: 0031, 0006, 0015, 0026, 0027, 0033, 0034, 0040, 0041, 0042, 0043) from what the RFC settles, and 0020 and 0028 are touched without being in that list. Resolved: `decided` holds the eleven decisions section 3 names, dropping 0020 and 0028.",
        "`rfc-2026-003.decided_by` names only 0041, but protocol 5 rests on the wire decision (0010, `Ship a static executor and treat the transport as a plug`), the version stamp (0040) and the sealed premise (0041). A reader tracing the contract to its decisions stops one step in. Resolved: `decided_by` lists 0010, 0040 and 0041.",
        "The thirteen decisions the proposal names all carry `related_rfcs: &[]`, so the chain proposal → decisions → specification is one-way. The wire decisions (0040, 0041) especially should carry `crate::rfc::rfc_2026_003`. Resolved: 0010, 0040 and 0041 carry it. The gate still does not hold them both ways, which stays open.",
        "`p0001.specification` narrows the contract to the wire, but RFC 0001 is `applying machines in parallel, with state in shards` — shards, the recorder, parallelism, the provider contract. Defensible because the wire is the one compatibility surface, but the narrowing is a judgment call nobody recorded. Resolved: the spec's goal says the wire is the one compatibility surface and the rest is internal.",
        "Four proptest cases share one `canonical-proptests` anchor because the scan cannot see inside `proptest!`, so an invariant's `tests` names the block and not the test. Resolved: the macro is split into four `#[test]` functions, each with its own anchor.",
        "The measurement: H1 holds — the compiled references are green and review still found five disagreements the build could not catch, so the compiled form catches renames and deletions while review catches the semantic mappings. H9 holds — the local types forced the statuses and the anchor references, and are not offered to Slipway.",
    ],
    authors: nonempty_str!["The reviewer"],
    decided_at: dacc_core::date!(2026, 9, 28),
);
