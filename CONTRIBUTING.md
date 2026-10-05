# Contributing

Read [RULES.md](RULES.md) first. It is eight prohibitions and one page, and each
of them breaks something a later commit cannot repair. Then
[CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md), which is short and has one paragraph
worth reading twice — review here is blunt about work and never about people.

**Found a vulnerability?** Not an issue and not a pull request:
[SECURITY.md](SECURITY.md) has a private channel and says what counts as one.

## Before anything else

The repository is private while the early stages are in progress, but the crates
are published to crates.io, so the source is public there. Once the repository
opens, the notes below apply.

Work lands on `main`, gated on every commit. A stage is declared done when its
exit criterion passes on real hardware, and a release is a tag at that boundary
(adr-2026-014) — the gate passing is necessary and not sufficient.

## Building and checking

Once per clone, before anything else:

```console
$ cargo xtask hooks
```

Git does not clone hooks, so until that runs, nothing is checking the commits
this working copy writes. What the hook checks and why:
[docs/COMMITS.md](docs/COMMITS.md).

Then, for the code:

```console
$ cargo xtask ci
```

That is the whole gate: formatting, clippy with warnings denied, both test
configurations, this repository's own rules, and the invariant registry. CI runs
the same command and nothing else, so what passes locally passes there.

Individual steps are available as `cargo xtask fmt`, `lint`, `test`,
`invariants` and `docs` when a full run is more than you need. `cargo xtask
mutants` and `cargo xtask budget` are not part of the gate: both want a release
build and an otherwise idle machine, so they run nightly.

Where anything goes: [docs/MAP.md](docs/MAP.md). What each rule is and why it
exists: [docs/CODING_STANDARDS.md](docs/CODING_STANDARDS.md). Which documentation
a change has to bring with it: [docs/DOCUMENTATION.md](docs/DOCUMENTATION.md).
Why anything is the way it is: [doc/adr/](doc/adr/).

Something noticed that is not yet a decision goes in
[docs/WATCHLIST.md](docs/WATCHLIST.md); a question nobody has to answer yet goes
in [docs/OPEN_QUESTIONS.md](docs/OPEN_QUESTIONS.md). Both are cheaper than a
decision record on purpose.

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
