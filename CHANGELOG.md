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

### Changed

- Workspace lints set to their strictest: `unsafe` forbidden, `missing_docs`
  denied, clippy `pedantic` and `cargo` denied, and targeted bans on `unwrap`,
  `panic`, indexing and lossy casts. Sixty-two findings, all fixed.

- Licence is Apache-2.0. The provider ecosystem has to be written by people who
  are not the author, and nobody writes a plugin while wondering whether it has
  become a derivative work.
