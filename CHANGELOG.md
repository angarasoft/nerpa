# Changelog

Notable changes, newest first. This project is pre-1.0 and interfaces change
without ceremony until the first stage is complete.

## Unreleased

### Added

- Resource addresses, with a slot that lets two versions of one declaration
  coexist and with no index anywhere in the scheme.
- Execution domains, read from an address rather than stored beside it.
- Resources: attributes, declared ordering, and whether drift on them may be
  overwritten or has to be merged by a person.
- Contract macros, on in release builds.
- The resource graph: staging by topological level, deterministic cycle
  reporting, and refusal of dangling or duplicated declarations.
- An invariant registry in `docs/invariants.toml`, checked in both directions
  against `// ENFORCES:` anchors in the code.
- `cargo-deny` configuration, with a licence allowlist that fails the build.
- `cargo xtask` as the single gate: formatting, clippy, both test configurations,
  repository rules and the invariant registry. CI runs it and nothing else.
- `docs/CODING_STANDARDS.md`, split into what the gate enforces and what review
  does.

- Decision records in `docs/decisions/`, immutable once accepted and superseded
  rather than edited. Eight of them, including the reversal that replaced lazy
  planning with a single pass.
- `docs/DOCUMENTATION.md`: which kind of documentation answers which question,
  where each lives, and what the gate checks.
- `cargo xtask docs`: decision numbering, required sections, both directions of
  every supersession, then rustdoc with warnings denied.

- Seven more decision records: zero trust, the executor and transports, observed
  state, the machine-facing surface, what is taken from Portage, how the project
  ships, and how it is tested.
- `docs/TESTING.md` and exit criteria in `ROADMAP.md` that can be executed rather
  than judged.

- Tests for the gate itself: every check now takes its input as data and is shown
  a fixture that breaks it, so no rule is trusted without having been watched
  fail.
- `cargo xtask mutants` and `cargo xtask deny`. Every tool the project uses is
  reached through `xtask`, including the ones living outside the workspace.
- Tests closing every gap the first mutation run found: the charset rule per
  alternative, `is_control` asserted both ways, `Graph::len`/`is_empty`/`get`,
  the cycle report against a graph holding resources it does not block, and the
  identifier rule one condition at a time.

- Unresolved values: an attribute holds either a settled value or a reference to
  the resource that will supply one. `Value` stays settled-only, because it is
  also what observed state is made of and an observed value is never unknown.
- An inventory saying where each node comes from, and edges derived from it: a
  resource on a node is ordered after whatever creates that node, and a resource
  on a node nobody declared is refused.
- Edges derived from references, so ordering no longer depends on remembering to
  declare it.

- `nerpa-config`: a Starlark host. `resource()` declares something that should
  exist and returns a handle; reading an attribute of it yields a value that will
  not exist until apply; `node()` says where a machine comes from.
- Three mechanisms holding one guarantee, because the toolchain allows only some
  of them to be precise. Ordering, indexing and iteration are refused by the
  value with a line. Equality and containment are refused by a pass over the
  syntax tree before evaluation, because Starlark answers them without consulting
  the value. Truthiness is recorded and the whole run refused.

- The planner: desired and observed state into a plan, in one walk of the graph.
  Creates, updates and replacements, with the attribute that forced a replacement
  named rather than left to be inferred.
- `Observed`, `Schema` and the plan types — `Plan`, `PlanStage`, `Change`,
  `AttributeDelta`, `Confidence`, `BlastRadius`. Ordered throughout, because a
  plan is going to be hashed and signed.
- References resolve against what already exists, which is what makes a second
  run empty.

- The provider boundary, and `nerpa-provider-os`: files and directories on the
  machine Nerpa runs on. Every provider must ship a fake, because the trait says
  so and a provider without one does not compile.
- `observe` and `apply`: reading the world once before planning, then walking the
  plan in the order it already fixed, filling in values an earlier stage produced.

- State: what the last run made, and what it depended on when it made it.
  Removals in reverse dependency order, taken from recorded edges because the
  configuration that described them is exactly what was deleted.
- `ChangeKind::Delete`, `Provider::delete`, and destroy-before-create for a
  replacement.
- A resource that moved is found where it was left rather than where the
  declaration now points, so changing a location is a replacement instead of a
  silent duplication.

- State on disk: JSON a person can read, written whole or not at all, behind a
  lock so two runs cannot each write what the other did not know about.
- The `nerpa` command: `plan` and `apply`, with plan output shaped after
  `emerge -pv` — fixed action column, deltas rather than values, summary before
  detail, the cause of a replacement named where it happens.
- `nerpa plan --json` prints the same object the text is rendered from.

- A machine that does not answer is refused per resource rather than per run.
  What could not be read appears in the plan saying so, with the machine and the
  age of the newest knowledge of it, and nothing is claimed about it.
- `ProviderError::Unreachable`, the only failure a run survives. Everything else
  still stops it, because everything else means something is wrong rather than
  merely out of reach.
- State records when each entry was written, and the format goes to version 2.

- Reads happen in batches, grouped by machine before anything is asked. Reading a
  machine takes one journey however many resources are on it.

- `nerpa-proto`: the wire between the engine and an executor on a machine.
  Length-prefixed CBOR frames, every one stamped with the protocol version it was
  written by, and a size that is checked before anything is allocated.

- A third kind of attribute: what a resource was born with. Editing an image
  reference no longer proposes to destroy the fleet, and the difference is
  reported on every plan rather than ignored.

- Project management, sized to this repository rather than to the one it is
  borrowed from ([0030](docs/decisions/0030-claim-less-and-attack-what-is-claimed.md)):
  `docs/MAP.md` says which kind of knowledge lives where and what has to move
  with what, `RULES.md` holds the eight prohibitions a later commit cannot
  repair, `docs/WATCHLIST.md` takes an observation for the price of a row, and
  `docs/OPEN_QUESTIONS.md` takes a question with the stage by which it is due.

- `cargo xtask budget`: what a plan costs, measured against `docs/budgets.toml`.
  A baseline refuses a regression against what the code already did; a budget
  reports the distance to what it ought to cost. Nightly, beside `mutants`, for
  the same reason.

- `cargo xtask docs` now also checks that every section of `ARCHITECTURE.md` says
  whether it is built, and that every link between written pages resolves. The
  second found five records linking to their neighbours from the repository root,
  which meant none of those links opened.

- A commit subject has a shape, and `.githooks/commit-msg` knows it — installed
  by `cargo xtask hooks`, checked by `cargo xtask commit-msg`, written down in
  `docs/COMMITS.md`. Twenty-five commits in, the history already spelled one type
  two ways: `[DOC]` twice and `[docs]` three times, for the same act.

### Changed

- The invariant registry says what is proven rather than what is hoped. Every
  built entry records the `bypass` it could suffer; `enforced` now requires
  `adversarial` tests that carry that bypass out, and `claimed` is the honest
  status for a guarantee nobody has attacked. Four entries moved to `claimed`,
  each with a reproduction: `INV-PLAN-001` and `INV-SECRET-004` (a handle or a
  secret laundered through a list or a dict reaches a comparison the syntax pass
  does not match — both since closed by tests named after those routes),
  `INV-PLAN-003` (an attribute the provider does not report back never settles,
  so no run is ever empty) and `INV-STATE-004` (the state rename is atomic
  against a stopped process, not a stopped machine). No code got worse; the count
  of demonstrated guarantees fell because it was being counted wrongly.

- `docs/ARCHITECTURE.md` marks each section `built`, `partly built` or `planned`.
  Eight of nineteen describe things nobody has built, which the present tense
  they are written in did not say.

- Renamed from `sarma` to `nerpa`, and the Gentoo package manager that held the
  name to `barguzin`. Speed was never the thing being sold; reach across domains
  is, and one of those names says so.

- Workspace lints set to their strictest: `unsafe` forbidden, `missing_docs`
  denied, clippy `pedantic` and `cargo` denied, and targeted bans on `unwrap`,
  `panic`, indexing and lossy casts. Sixty-two findings, all fixed.

- Licence is Apache-2.0. The provider ecosystem has to be written by people who
  are not the author, and nobody writes a plugin while wondering whether it has
  become a derivative work.
