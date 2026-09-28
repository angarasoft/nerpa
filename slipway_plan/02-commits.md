# 02. Commits in nerpa and in Slipway

D2 is decided: option B. A project adopting a methodology keeps the meaning its
history relies on and takes the methodology's mechanics, so the convention is the one
nerpa needs, carried by Slipway's form, basis and tools.

The comparison below is what the decision was taken from. The rules are compared as
the code enforces them — nerpa's [`xtask/src/commit.rs`](../xtask/src/commit.rs) and
[COMMITS.md](../docs/COMMITS.md), Slipway's
[`message.rs`](https://github.com/anishukserg/slipway/blob/master/crates/slipway-cli/src/message.rs)
and [decision 8](https://github.com/anishukserg/slipway/blob/master/doc/adr/a0008.rs)
— rather than as either document describes them.

## The shapes

```text
nerpa today    [TYPE] subject
               [TYPE] scope: subject

Slipway        [TYPE](scope): subject
               [TYPE](scope,scope): subject

               body

               Slipway-Work: w0001

Decision B     [CHANGE](core): stop resolving a reference to a value that is changing

               body

               Slipway-Work: w0012

Written by Slipway's own tool, today
               [PLAN](area): <"work started", in Russian> w0001
```

## Side by side

| | nerpa | Slipway |
| --- | --- | --- |
| Types | FEAT, FIX, PERF, CHANGE, DOCS, TEST, CI, CHORE | FEAT, FIX, REFACTOR, TEST, DOCS, ADR, PLAN, CHORE |
| Scope | Optional; any word without a space — `os`, `nerpa-cli`, `0023` | Required; one or more subsystems of the registry's taxonomy, lower case, `a-z`, `0-9` and `,` only |
| Length | Over 72 is mentioned, over 90 refused | Over 72 refused |
| Full stop at the end | Refused | Refused |
| Blank line after the subject | Required | Required |
| A wrong type | Answered with the right one: `[DOC]` becomes `[DOCS]`, `[REFACTOR]` becomes `[CHANGE]`, and the wrong case is named | Answered with the list |
| Basis | None | `Slipway-Work: wNNNN` on every commit, and the work item must exist in the commit's own tree; closing a slice carries `Slipway-Slice: sNNNN` instead |
| `fixup!`, `squash!`, `amend!`, `Revert "…"` | The wrapper is stripped and the subject inside is judged | Not recognised, so refused |
| Merge commits | Accepted while git is merging | Refused; merge buttons are forbidden (decision 18) |
| Making the commit | `git commit`; the hook judges the message | `cargo slipway commit -F <message> -- <paths>`: paths named explicitly, a lock, the full gate on the commit's own tree; `git add -A` and `commit -a` are forbidden |
| Hooks | `commit-msg` only, installed by `cargo xtask hooks`; skipped with a note when cargo is missing | `pre-commit` (the gate), `commit-msg`, `pre-push` (dependency policy on a fresh database, archive branches, external names); a missing tool is a refusal |
| In CI | Not checked | `msg-check --range` over every pushed range |
| Commits the tool writes | None | `work start`, `land` and `drop`, `slice close` and `journal import` commit journal events, with Russian subjects and bodies |
| Bypassing | Rule 8: never | `--no-verify`, amending and rewriting history only at the owner's explicit request |

## Where the difference is meaning, not spelling

1. **CHANGE is not REFACTOR.** nerpa refuses `REFACTOR` on purpose: `CHANGE` says
   behaviour is different because somebody decided it should be — a default, a
   name, a boundary — and `REFACTOR` hides exactly that. Slipway has `REFACTOR` and
   no `CHANGE`. 14 of nerpa's commits are `CHANGE`, and 9 of those changed decision
   records.
2. **PERF and CI have no Slipway type.** They would become `REFACTOR` or `FEAT`, and
   `CHORE` — the distinctions nerpa's closed list was written to keep.
3. **DOCS splits in three.** Slipway gives decision records `ADR` and the plan and
   its journal `PLAN`. In nerpa a commit touching a record is typed by what the
   record did: 9 `CHANGE`, 9 `FIX`, 11 `DOCS`, 4 `FEAT`.
4. **No commit without planned work.** The largest difference is not in the subject
   at all. A typo, a dependency bump and a watchlist row each need a work item in a
   current slice — five at most per slice — or a `Toil` item that justifies itself.
5. **The scope becomes a classification.** About 84 of 121 nerpa subjects have no
   scope. Under Slipway every commit names subsystems, and the subsystem is where
   conformance levels and radius are declared.
6. **Git's own tools stop fitting.** `git revert`, `commit --fixup` and merges
   produce subjects Slipway refuses.
7. **Language.** The subjects and bodies the tool writes are Russian today. This
   repository stays English (L3), so that changes in Slipway before the switch (F10).

## Decision B, part by part

| Part | Taken from | Why |
| --- | --- | --- |
| Types | nerpa — FEAT, FIX, PERF, CHANGE, DOCS, TEST, CI, CHORE — plus ADR and PLAN | CHANGE, PERF and CI are the distinctions nerpa's closed list exists to keep. ADR and PLAN are added because the registry makes a decision record and a change to the plan kinds of commit of their own, and Slipway's tool writes PLAN |
| A wrong type | nerpa — `[DOC]` answered with `[DOCS]`, `[REFACTOR]` with `[CHANGE]` | The answer is the useful half of the check |
| Length | nerpa — mentioned over 72, refused over 90 | `docs/COMMITS.md` tightens a limit by bringing practice down first, and 4 of the last 50 subjects are over 72 |
| Language | nerpa — English, including what the tool writes | [DOCUMENTATION.md](../docs/DOCUMENTATION.md): English in the repository, all of it |
| Form | Slipway — the scope in parentheses | `msg-check` reads it |
| Scope | Slipway — required, subsystems of the taxonomy | It ties every commit to a subsystem, where conformance and radius are declared |
| Basis | Slipway — `Slipway-Work`, or `Slipway-Slice` when closing a slice | What the pilot tests |
| Making a commit, hooks, CI | Slipway — explicit paths, the gate on the commit's own tree, `msg-check --range` | What the pilot tests |

## What B needs from Slipway

The types, their hints and the subject limits are constants in `message.rs`. B needs
them declared by the product's registry, the way subsystems already are (F3).

The commits the tool writes itself — starting, landing and dropping work, closing a
slice, importing history — are Russian strings in `work.rs`, and so is the gate's
verdict it stores in journal events (F10). They take the product's language in the
same change, so that everything B puts into this repository is English.

The switch waits for both, and there is no fallback to A: A would drop the meaning B
was chosen to keep.

## Rejected

- **A — Slipway's rules as they ship.** Drops CHANGE, PERF and CI, refuses subject
  lengths nerpa's history shows are normal, writes Russian into the history, and would
  make nerpa spell its history to fit another project's constants.
- **C — nerpa's rules, plus the trailer.** Two message checkers that disagree,
  subjects written by `cargo slipway work` refused by nerpa's own hook, and the
  weakest test of Slipway there is.

## Branch flow and CI (L2)

Decided: the gate runs locally until the switch, and on GitHub from the switch on.

- Before the switch no commit carries a basis, so nothing Slipway promises is being
  claimed while the gate is local.
- From the switch commit on, the GitHub repository named in `Cargo.toml` is where the
  gate runs, with `cargo xtask ci` and `cargo slipway msg-check --range` required on
  one protected branch ([04](04-layout.md)).
- nerpa has never had a merge, a revert or a fixup. The branch stays linear and takes
  fast-forward pushes only, and Slipway learns to accept `Revert "…"` of an accepted
  subject before the first revert is needed (F11 in [07](07-slipway-feedback.md)).

Becomes: a decision on the commit convention in `doc/`, with `docs/COMMITS.md`
rewritten for it; findings F3, F10 and F11 in Slipway.
