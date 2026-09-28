# Slipway transition plan

How nerpa moves onto [Slipway](https://github.com/anishukserg/slipway), as the
first project outside Slipway itself to run on it.

- Status: draft — nothing in it has been carried out
- Date: 2026-09-11
- Baseline: nerpa `d11060f`, Slipway `d8f0640`
- Owner and verdict: Anishchuk Sergei, after an independent review on record (D6)

This directory is a seed rather than a home. Each file names the records it turns
into in `doc/` — the Slipway registry of this repository, which does not exist yet —
and the work item that finishes moving a file there deletes it. Until then this is
the one place the transition is planned, and a second copy of it anywhere else would
be the drift the pilot exists to measure.

Written in English, as everything in this repository is
([DOCUMENTATION.md](../docs/DOCUMENTATION.md)). What is filed in Slipway's own
registry is written in Russian, the language of that repository.

## Decisions taken

| # | Question | Answer | Argued in |
| --- | --- | --- | --- |
| D1 | Where the pilot protocol lives | In this repository, as the thrust `slipway_pilot` of the Slipway registry at `doc/`, which also receives the product's records as the waves convert them | [04](04-layout.md) |
| D2 | The commit convention | Option B: the vocabulary and limits nerpa needs, in Slipway's form, with Slipway's basis and mechanics | [02](02-commits.md) |
| D3 | The existing decision records and RFCs | Converted, not frozen, onto document types standardised first | [03](03-document-types.md) |
| D4 | How Slipway's crates arrive | Git dependencies pinned to one revision behind one exact `allow-git` entry; publication to crates.io waits for the verdict | [04](04-layout.md) |
| D5 | How much of the project | All of it, one wave at a time, and one subsystem at a time inside a wave | [05](05-sequence.md) |
| D6 | Who decides whether it worked | The owner, against criteria written before the start, after a review by a different model kept as a record | [06](06-hypotheses.md) |

## Details settled

| # | Detail | Decided | Argued in |
| --- | --- | --- | --- |
| L1 | Where the registry lives | At `doc/`, beside `docs/`, where `cargo slipway` looks for it, so Slipway's layout needs no change | [04](04-layout.md) |
| L2 | Where the gate runs | Locally until the switch onto Slipway's rules; on GitHub, required on one protected branch, from the switch on | [02](02-commits.md), [04](04-layout.md) |
| L3 | The language of this repository under Slipway | English, with no exception. Slipway is a methodology and its schema is language-neutral; the Russian its tool writes into commits and journal events is changed in Slipway before the switch (F10) | [02](02-commits.md), [07](07-slipway-feedback.md) |

## Files

| File | What it settles | Becomes, in `doc/` |
| --- | --- | --- |
| [01-baseline.md](01-baseline.md) | Where both projects stand, and what is measured before the start | Baseline measurement records |
| [02-commits.md](02-commits.md) | How nerpa's and Slipway's commit rules differ, and decision B | A decision on the commit convention |
| [03-document-types.md](03-document-types.md) | The document types, and how each existing document converts | A decision on document types; one decision per local type |
| [04-layout.md](04-layout.md) | The registry's place, delivery, lints, gate, CI and hooks | Decisions on layout, delivery and the gate |
| [05-sequence.md](05-sequence.md) | The waves, the switch, and the order of subsystems | The thrust `slipway_pilot`, one slice per wave |
| [06-hypotheses.md](06-hypotheses.md) | What is measured, what stops the pilot, how the verdict is given | The pilot's first decision; an anonymised copy closes Slipway's w0027 |
| [07-slipway-feedback.md](07-slipway-feedback.md) | How findings reach Slipway, and what is expected to be found | Work items here; anonymised work items under Slipway's t0004 |
| [08-risks.md](08-risks.md) | What can go wrong, and how the pilot is undone | Trade-offs of the decisions above |

## Changing the plan

- A change is an ordinary commit, and says which decision it moves.
- Links are relative or `https://`. `cargo xtask docs` resolves every route in every
  Markdown file, and a local absolute path does not resolve.
- nerpa is named freely here. Nothing is copied from here into Slipway without the
  anonymising described in [07](07-slipway-feedback.md): Slipway's decision 9
  forbids the name there, and its gate refuses it.
