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

- `sarma-config`: a Starlark host. `resource()` declares something that should
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

### Changed

- Workspace lints set to their strictest: `unsafe` forbidden, `missing_docs`
  denied, clippy `pedantic` and `cargo` denied, and targeted bans on `unwrap`,
  `panic`, indexing and lossy casts. Sixty-two findings, all fixed.

- Licence is Apache-2.0. The provider ecosystem has to be written by people who
  are not the author, and nobody writes a plugin while wondering whether it has
  become a derivative work.
