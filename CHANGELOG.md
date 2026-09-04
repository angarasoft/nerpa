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
