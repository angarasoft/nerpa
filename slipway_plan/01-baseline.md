# 01. Baseline

Where both projects stand on 2026-09-11. Every hypothesis in
[06](06-hypotheses.md) is measured against this, so it is written down before
anything changes.

## nerpa at `d11060f`

### Repository and CI

- One branch, `develop`, and a linear history: no merge commit, no revert and no
  fixup, ever.
- One remote, `gitlab`, 98 commits behind; its `main` holds only the initial
  commit.
- The workflow in `.github/workflows/ci.yml` has no GitHub remote to run for in
  this clone, and GitLab has no pipeline file. The gate runs when somebody runs
  `cargo xtask ci`, and nowhere else.

Slipway states that it does not work without a protected branch and required CI
([GUARANTEES.md](https://github.com/anishukserg/slipway/blob/master/GUARANTEES.md)).
Decided (L2): the gate stays local until the switch onto Slipway's rules, and runs on
GitHub from the switch on ([04](04-layout.md)).

### Documents

| Kind | Count | Carrier | Held by |
| --- | --- | --- | --- |
| Decision records | 43 — 42 accepted, 1 superseded; 2 706 lines | `docs/decisions/` | `cargo xtask docs` |
| — carrying a `Corrected:` line | 9 | | |
| — with sections beyond Context, Decision and Consequences | 5 | | |
| RFCs | 3 — 0001 in review at revision 4, 0002 and 0003 drafts | `docs/rfcs/` | review, logged in each |
| Invariants | 58 | `docs/invariants.toml` and `// ENFORCES:` anchors | `cargo xtask invariants` |
| Watchlist rows | 24, live and closed | `docs/WATCHLIST.md` | review |
| Open questions | 0 open, 5 answered | `docs/OPEN_QUESTIONS.md` | review |
| Roadmap stages | 7, of which 0 and 1 are met | `ROADMAP.md` | their exit criteria |
| Links from documents to decision records | 97 | | `cargo xtask docs` |
| References from code to `docs/decisions` | 26, and 9 `Expands:` joins | | `cargo xtask book` |

Counted over the tree, for example:

```console
$ ls docs/decisions/0[0-9]*.md | grep -vc 0000-
$ grep -l '^- Corrected:' docs/decisions/*.md | wc -l
$ grep -c '^id = ' docs/invariants.toml
$ git log --merges --oneline | wc -l
```

### Commits

121 commits when counted at `1a71eec`:

| Type | Commits |
| --- | --- |
| FIX | 36 |
| DOCS | 32, and 5 in older spellings |
| FEAT | 30 |
| CHANGE | 14 |
| TEST | 3 |
| PERF | 1 |

About 37 carry a scope. 4 of the last 50 subjects are longer than 72 characters.

### Open defects

Found by the external review of 2026-09-11 and each reproduced by a test that was
never added to the repository. Number 4 was fixed in `d7330a9`; the other seven were
open at `1a71eec` and are checked again before the slice holding them is cut:

1. a service declared without `enabled` or `running` is disabled or stopped when it
   is updated;
2. removing a service's declaration stops and disables the service;
3. a reference to an attribute that changes in the same plan resolves to the old
   value, so the next run is not empty;
5. a package or service on a machine lost after it was opened stops the whole read;
6. `mode = "644"` never converges;
7. the state file is written readable by everybody, digest key included;
8. a control-domain resource behind an unreachable machine disappears from the
   plan.

They are the first product slices in [05](05-sequence.md).

### Measured before the switch

- `cargo xtask ci`, cold and warm, wall clock;
- commits per week over the two weeks before the start;
- the records a change touches — decision, invariant, changelog, roadmap,
  architecture — over the last 20 changes, which is the baseline of H7.

## Slipway at `d8f0640`

- Published at https://github.com/anishukserg/slipway, branch `master`. Slice s0010
  is closed: the gate and a check of each commit's basis run in GitHub Actions
  (decision 18).
- Built: a registry of decisions and specifications with references the compiler
  checks (`adr!`, `rfc!`, the scan, `#[doc_anchor]`); the minimal work layer —
  thrust, slice, work item and its origin; the journal, folded when the registry
  builds; `cargo slipway` with `commit`, `msg-check`, `gate`, `hooks`, `work`,
  `slice` and `journal`.
- Not built: the access layer (`map`, `where`, `survey`, `drift`, `conformance`),
  the gate catalogue, baselines, owners, the debt ledger, the incoming flow, a
  configurable layout, and rendering the registry to Markdown.
- Adopted nowhere outside itself, and its README and GUARANTEES say so.
- The pilot is already in its plan:
  - thrust [t0004](https://github.com/anishukserg/slipway/blob/master/doc/thrust/t0004.rs):
    exit criteria in the registry before the pilot starts, and a verdict by them;
  - slice [s0008](https://github.com/anishukserg/slipway/blob/master/doc/slice/s0008.rs):
    what blocks a pilot at L1;
  - w0025, landed: generated code under a strict lint profile;
  - [w0026](https://github.com/anishukserg/slipway/blob/master/doc/work/w0026.rs),
    open: how the crates reach a project whose dependency policy allows only
    crates.io;
  - [w0027](https://github.com/anishukserg/slipway/blob/master/doc/work/w0027.rs),
    open: hypotheses and exit criteria, as a decision taken before the start.
- [Decision 9](https://github.com/anishukserg/slipway/blob/master/doc/adr/a0009.rs):
  Slipway's repository names no external project, held by its gate and `pre-push`
  against a local list. nerpa cannot be named there.

Becomes: the baseline measurement records in `doc/`, filed under the thrust
`slipway_pilot` before its first slice starts.
