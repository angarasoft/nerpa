# 07. Findings for Slipway

The pilot exists as much to test Slipway as to move nerpa, and a finding that stays
in this repository has done half its job.

## The route

1. **Recorded here at once**, in `doc/`, under the thrust `slipway_pilot`:
   - a Slipway promise that did not hold is a `Divergence` from the pilot's
     specification, whose invariants state the promises under test;
   - friction, a missing mechanism or a question is an `Inquiry`, or an observation
     once that type exists;
   - a number is a measurement work item.
2. **Carried to Slipway when a pilot slice closes**, anonymised:
   - no project name, crate, host, invariant key, record number or quotation of a
     nerpa document, as Slipway's decision 9 requires;
   - the failure told in Slipway's own terms, with a reproduction that does not need
     nerpa;
   - filed under t0004: a `Divergence` from Slipway's r0001 or r0002 where a promise
     failed, an `Inquiry` for a design question, a `Decision` once decided;
   - written in Russian, the language of that repository.
3. **Guarded before the first such commit:** nerpa goes into Slipway's local
   external-names list and its CI secret, so that its gate and `pre-push` refuse a
   leak.
4. **Issues on GitHub** only for what a reader outside needs, anonymised the same way.
5. **Back in nerpa**, a fix arrives as a new pinned revision: one commit, one work
   item, and the finding closed with the revision that fixed it.

## Language

Slipway is a methodology, and nothing in its schema is tied to a language: record
types, field names, variants, commit form, trailer keys and journal keys are English
identifiers, and prose is whatever the product writes. nerpa therefore stays English
(L3). What is tied to Russian is the tool, in three places that write into a
product's repository and two that do not:

| Where | Written into the repository | Answer |
| --- | --- | --- |
| Commits made by `work`, `slice` and `journal import` | Yes: subjects and bodies | F10, before the switch |
| The gate's verdict stored in `gate` journal events | Yes: inside `doc/journal/` | F10, before the switch |
| Hooks written by `hooks install` | Yes: a comment in each hook | Hooks are written by hand here |
| Messages on the terminal | No | F10, not blocking |
| Documentation generated for the registry crate | No: produced at build time | F10, not blocking |

## The probe

Findings that running the tool could settle were checked on 2026-09-11 against
Slipway `d8f0640`, on a probe that names no project and can be handed to Slipway as it
is: a two-crate workspace on edition 2024 — a registry with one decision,
specification, thrust, slice and work item, and a product with one anchor — using
Slipway's crates by path, under a strict lint profile: `clippy::pedantic`,
`missing_docs`, `unreachable_pub`, the rustdoc link lints, and the restriction lints
`unwrap_used`, `panic`, `indexing_slicing`, `as_conversions` and `print_stdout` among
them. The one allowance the probe needed was its own build script printing to cargo.

## Findings

| F | What | Checked | Blocks |
| --- | --- | --- | --- |
| F1 | The gate's doctest floor is a constant, 36 | In code: `DOCTEST_FLOOR` and the check against it in `slipway-cli/src/gate.rs` | Proof of done in any other project |
| F2 | The gate's steps are fixed, and a product cannot add its own | In code: the twelve steps of `slipway-cli/src/gate.rs` | Proof of done; a single entry point |
| F3 | The commit types, their hints and the subject limits are constants, so a product cannot declare its own | On the probe: `[CHANGE]` and an 81-character subject refused | The switch: convention B is decided ([02](02-commits.md)) |
| F4 | `hooks install` leaves an existing hook in place and reports success, and the rule is never installed | On the probe: an existing `commit-msg` left as it was, exit status 0 | Silent loss of a rule |
| F5 | The registry root is fixed at `doc/` | In code: `slipway-cli/src/layout.rs`; decision 14 | Nothing: accepted (L1), and filed only if `doc/` beside `docs/` proves a friction |
| F6 | The crates are not on crates.io | In Slipway's plan: w0026 | Wave 0 |
| F7 | A product's restriction lints are not guaranteed on generated code | On the probe: **not confirmed** — clippy and rustdoc passed with warnings denied; dropped | — |
| F8 | A slice and a divergence need a specification inside the registry, and there is no baseline | In the schema: `slipway-work/src/schema.rs` | The first product slice |
| F9 | The access layer is not built, yet `survey` is offered as the first step | On the probe: `survey` is an unknown command | Hypotheses about agents |
| F10 | The text the tool writes is Russian only: commits it makes, the gate's verdict in journal events, installed hooks, terminal messages, generated documentation | On the probe: `work start` committed a Russian subject into an English registry; the hooks it wrote and the code it generated carry Russian | The switch: this repository stays English (L3) |
| F11 | `Revert "…"`, `fixup!` and merge subjects are refused | On the probe: `Revert` and `fixup!` subjects refused | Git's own tools |
| F12 | Consumers on edition 2024 are untested | On the probe: **not confirmed** — edition 2024 builds, lints and documents cleanly; dropped | — |
| F13 | "RFC" names a specification, and there is no proposal type | In the schema: `slipway-knowledge/src/schema.rs` | Wave 3 |
| F14 | There is no review type, so independence has no carrier | In the schema | The verdict; independence by radius |
| F15 | There is no typed invariant registry | In r0002 | Wave 3 |
| F16 | A decision may carry no trade-offs, so its form does not require a cost | In the schema: `slipway-knowledge/src/schema.rs` | Wave 1 |
| F17 | A decision has no field for corrections, and a supersession is recorded at one end only | In the schema: `slipway-knowledge/src/schema.rs` | Wave 1 |
| F18 | History from before commits carried a basis cannot be journaled, and a thrust has no closing event | In code: decision 15 and `journal import` | Wave 2 |
| F19 | There is no observation or incoming-flow type | In SLIPWAY.md | Wave 5 |
| F20 | The registry cannot be rendered to Markdown | In SLIPWAY.md | The book and the site |
| F21 | Anchor ids are unique, so one invariant held at many sites needs an id per site | In code: `slipway-scan/src/anchors.rs` | Wave 3 |
| F22 | `landed` needs a proof recorded by the hook on the machine that made the commit | On the probe: `work land` refused without a local proof | Agents working in remote sessions |
| F23 | A refused commit message is pointed at Slipway's commit rules as `doc/adr/a0008.rs`, which in a product's registry is that product's own decision 8 or nothing | On the probe: every refusal names `doc/adr/a0008.rs`, absent from the probe's registry | Misleading rather than blocking: nerpa's `a0008` will be a different decision |

Becomes: a work item in `doc/` for each confirmed finding, and an anonymised work item
under Slipway's t0004.
