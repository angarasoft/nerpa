# Contributing

## Before anything else

The repository is private while stage 0 is in progress. Once it opens, the
notes below apply.

## Building and checking

```console
$ cargo xtask ci
```

That is the whole gate: formatting, clippy with warnings denied, both test
configurations, this repository's own rules, and the invariant registry. CI runs
the same command and nothing else, so what passes locally passes there.

Individual steps are available as `cargo xtask fmt`, `lint`, `test` and
`invariants` when a full run is more than you need.

What each rule is and why it exists: [docs/CODING_STANDARDS.md](docs/CODING_STANDARDS.md).

## Two layers of correctness

State what you can in the type system, and what you cannot in a contract.

A type is preferred because it costs nothing at runtime and cannot be forgotten:
`Address` cannot hold a component starting with `-`, so no interpolation
mistake anywhere else can turn a resource name into a command line option.

For everything else there are `requires!`, `ensures!` and `invariant!`. They stay
compiled into release builds on purpose — the runs that matter are unattended,
from release binaries, against production, which is exactly where
`debug_assert!` would be gone. A contract failure panics, and that is the safe
direction: the process holds a belief about a machine that reality does not
share, and the next thing it would do is act on it.

## Comments

Explain the reasoning and what breaks without it, not what the line does. A
comment that survives is one that stops somebody from "simplifying" a decision
back out six months later.

## Copyright

Contributions are accepted under a contributor licence agreement.

This is not bureaucracy for its own sake, and it is worth being blunt about what
it is for. The engine is Apache-2.0 and stays Apache-2.0. The planned
coordination plane — grants, third-party approval, the audit chain — is intended
to be a separate component under a different licence, and offering that requires
the author to hold the rights to the whole work. A CLA is the only part of this
that cannot be added afterwards: without one, the first merged patch settles the
question permanently.
