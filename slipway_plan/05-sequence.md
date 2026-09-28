# 05. Sequence

D5: all of nerpa, one wave at a time, and one subsystem at a time inside a wave. A
wave ends on its exit criterion, not on a date. A subsystem's conformance level is
declared by a decision and only rises.

## Taxonomy

Subsystem names are what commit scopes are spelt with, so they use `a-z` and `0-9`
only.

| Subsystem | Covers |
| --- | --- |
| `Core` | `nerpa-core` |
| `Config` | `nerpa-config` |
| `Proto` | `nerpa-proto` |
| `Executor` | `nerpa-executor` |
| `Transport` | `nerpa-transport` |
| `Os` | `nerpa-provider-os` |
| `Cli` | `nerpa-cli` |
| `Gate` | `xtask`, CI, hooks |
| `Docs` | the book, the overview, the guides |
| `Pilot` | the pilot itself, the thrust `slipway_pilot`: protocol, measurements, findings |

Team: `Maintainers`, one person today. The axis exists so that ownership is written
down rather than assumed.

## Wave 0 — prerequisites, ending in the switch

In Slipway, anonymised as [07](07-slipway-feedback.md) describes:

1. nerpa added to the local external-names list and to the CI secret, before any
   commit about the pilot;
2. w0027 lands a decision holding the hypotheses and stop criteria of
   [06](06-hypotheses.md);
3. w0026 lands a decision on delivery, as D4 answered it;
4. a new slice under t0004 for a project with a registry of its own:
   - commit types, their hints and the subject limits declared by the product (F3),
     and the text the tool writes into commits and journal events in the product's
     language (F10) — the switch waits for both;
   - the doctest floor (F1) and a step for the product's gate (F2) — proof of done
     waits for them;
   - installing hooks over an existing one (F4).

In nerpa, under its current rules — these are the last commits without a basis:

5. the baseline of [01](01-baseline.md) measured;
6. `doc/` created: the taxonomy; the thrust `slipway_pilot`; the pilot's
   specification, whose invariants state the Slipway promises under test in English,
   each naming the row of GUARANTEES.md it translates; the pilot's decision
   ([06](06-hypotheses.md)); decisions on document types
   ([03](03-document-types.md)), the commit convention ([02](02-commits.md)), and
   layout, delivery and gate ([04](04-layout.md));
7. the lint spike;
8. rows for `doc/` in `docs/DOCUMENTATION.md` and `docs/MAP.md`, and `docs/COMMITS.md`
   rewritten for convention B.

Then **the switch**, one commit:

9. Slipway pinned at the revision carrying F3 and F10; hooks rewritten by hand, in
   English, with `commit-msg` calling `cargo slipway hook commit-msg`; the GitHub
   remote in place, with the gate and `msg-check --range` required on one protected
   branch ([04](04-layout.md)). Every commit after it carries a basis.

**Exit:** the switch has happened. The registry builds under nerpa's lints; CI on
GitHub accepts a commit with a valid basis and refuses one without; every pilot
record existed before the switch.

## Wave 1 — decisions, across the whole project

1. A converter in xtask that works both ways, Markdown to registry and back, with a
   round-trip test and a verbatim comparison of every section.
2. The 43 records become `a0001` to `a0043`; their subsystems are classified; the
   five records with extra sections are decided one by one, in a review record.
3. `cargo xtask docs` and `cargo xtask book` read the registry; the 97 document links
   and 26 code references point at registry records; `Expands:` joins registry ids.
4. The review type, which D6 needs and every independent reading after it uses.
5. The first anchors, on what must not change silently: `FORMAT_VERSION`,
   `PROTOCOL_VERSION`, address parsing, the `Provider` trait, `Operation` — named by
   the decisions that settled them.
6. `docs/decisions/` emptied in one commit, once the comparison passes.

**Exit:** every decision is a registry record and the Markdown carrier is gone;
`cargo xtask ci` is green; H8 measured.

## Wave 2 — the plan

1. Stages 2 to 6 of `ROADMAP.md` become thrusts. Stages 0 and 1 stay where they are,
   as history: a journal cannot hold what happened before commits carried a basis
   (F18).
2. A baseline specification whose invariants are the `INV-` entries the first slices
   touch, so that a slice has a specification and a divergence has something to
   diverge from (F8).
3. The seven open defects of [01](01-baseline.md) as `Divergence` work items, in two
   slices of at most five.

**Exit:** the product's current work — the seven divergences and the roadmap's open
stages — is in the registry; H2 and H4 measured.

## Wave 3 — proposals and invariants

1. The proposal type. RFCs 0001 to 0003 become `p0001` to `p0003` with bodies; their
   review rounds become review records; their decided parts are checked against the
   existing decisions; each accepted proposal yields a specification.
2. The invariant type. 58 entries converted; `// ENFORCES:` becomes anchors;
   `cargo xtask invariants` reads the registry; `docs/invariants.toml` is removed.

**Exit:** no row of `docs/DOCUMENTATION.md` points at a Markdown carrier that has
been converted; H1 and H9 measured.

## Wave 4 — proof of done, one subsystem at a time

Needs F1 and F2 from Slipway. Each subsystem is declared L2 by a decision; every work
item lands with a gate proof; independence required by a change's radius is a review
record.

| Order | Subsystem | Why here |
| --- | --- | --- |
| 1 | `Os` | Open divergences with a local radius: a short loop to learn the proof on |
| 2 | `Core` | The remaining divergences, then step 1 of RFC 0001 — state in shards is a `Persistent` change, the first whose radius sets the rigour |
| 3 | `Proto`, `Executor`, `Transport` | The wire, a contract with installed executors; RFCs 0002 and 0003 |
| 4 | `Config` | The language users write against |
| 5 | `Cli` | The surface over everything above |
| 6 | `Gate`, `Docs` | Last, because they are the pilot's own instruments |

**Exit, per subsystem:** two slices landed with proofs.

## Wave 5 — observations, questions, rules and guides

Watchlist rows become observations; open questions become `Inquiry` items or
observations; the rules become invariants of the process; the guides keep their prose
and lose whatever a decision or an invariant now holds.

## The verdict

Given when `Core` reaches L2, or eight weeks after the switch, whichever comes first
— the first point at which every hypothesis has data. The waves after it run only if
the verdict says to continue ([06](06-hypotheses.md)).

Becomes: the thrust `slipway_pilot` in `doc/`, with one slice per wave.
