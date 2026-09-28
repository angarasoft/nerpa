# 08. Risks and rollback

| Risk | Where it bites | Answer |
| --- | --- | --- |
| Conventions switched in the middle of a change | RFC 0001 is at revision 4, with a round of review pending | Wave 0 starts at a boundary: that round closed and the baseline measured |
| The gate is local until the switch | Slipway does not work without required CI (GUARANTEES.md) | Before the switch no commit carries a basis, so nothing Slipway promises is claimed; from the switch commit on, CI on GitHub is required |
| Two gates, two verdicts | nerpa keeps one entry point | Slipway's gate runs `cargo xtask ci` as the product's step (F2); until then CI runs both, and H3 measures the cost |
| Two carriers of one kind | A decision in Markdown and in the registry at once | A wave converts one type completely and deletes its Markdown carrier inside the same wave |
| Rule 1 read as forbidding conversion | Records that are immutable once accepted | Text moves byte for byte, generated and compared, and converting is itself a decision |
| Slipway holds nerpa up | F3 and F10 hold the switch; F1 and F2 hold proof of done | Convention B and an English repository have no fallback, so F3 and F10 are the first Slipway items of wave 0; nothing is started before it can land |
| Work stuck without a proof | A work item started before the gate can record a proof can never land | No work item is started before it can land; earlier work carries its trailer and is imported later as history |
| Tool commits crowd the history | `start` and `land` each make a commit | Measured by H7; a narrowed verdict can keep the registry and drop the journal |
| `doc/` read as `docs/` | Two directories one letter apart | `docs/DOCUMENTATION.md` and `docs/MAP.md` say which answers what; a confusion that happens anyway is an observation (F5) |
| The auditor is the audited | One author | A review by another model before the verdict ([06](06-hypotheses.md)) |
| nerpa's name reaches Slipway | Slipway's decision 9 | The external-names list and the CI secret come before the first commit about the pilot |
| Russian text in an English repository | [DOCUMENTATION.md](../docs/DOCUMENTATION.md) | No exception: the switch waits for F10, and until then the tool writes nothing here |

## Undoing the pilot

The steps that cannot be taken back wait for the verdict — Slipway's crates stay
unpublished and no subsystem is declared L3 — so that undoing the pilot stays cheap.

- **Before wave 1 lands:** remove `doc/`, the switch's hooks and CI jobs, and restore
  the rules of `docs/COMMITS.md`. Trailers already in the history are harmless.
- **Once wave 1 has landed:** also run the converter back, from the registry to
  Markdown. It is written to work both ways, with a round-trip test, for exactly this
  case.
- **Once wave 3 has landed:** the same, for proposals and invariants.
- **Recorded:** a decision to stop supersedes the pilot's decision.

Becomes: the trade-offs of the pilot's decision and of the decisions in
[04](04-layout.md).
