# Contributing

## Before anything else

The repository is private while stage 0 is in progress. Once it opens, the
notes below apply.

## Building and checking

```console
$ cargo test
$ cargo fmt --all -- --check
$ cargo clippy --all-targets --all-features
```

CI runs all three with warnings denied, so a warning is a failure.

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

Contributions are accepted under a contributor licence agreement. This is not
bureaucracy for its own sake: sarma is AGPL-3.0 with commercial licences
available, and offering those requires the author to hold the rights to the
whole work. Without a CLA that option closes the first time a patch is merged.
