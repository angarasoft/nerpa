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
