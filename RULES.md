# Rules

Read this first. Eight prohibitions, and breaking any of them damages something
that cannot be repaired by the next commit: the repository's history, a state
file on somebody else's machine, or the reason anybody believes what this project
says about itself.

Each rule is complete where it stands. Following it never requires opening
another file; the links are there for depth, not for compliance.

Everything that is merely important lives elsewhere —
[docs/CODING_STANDARDS.md](docs/CODING_STANDARDS.md) for how code is written,
[docs/MAP.md](docs/MAP.md) for where things go. This file is only for what must
not happen.

---

**1. An accepted decision record is never edited.**

Supersede it with a new one and make both point at each other. The records are
how a reversal stays legible a year later; editing one rewrites the past into
something that was always right, and the reasoning that made the reversal
necessary disappears with it.
Depth: [docs/decisions/README.md](docs/decisions/README.md).

**2. The release profile never sets `panic = "abort"`.**

A run holds a state lock and, later, an open transport. Contracts are compiled
into release builds and stop the process when a belief about infrastructure turns
out to be false. Unwinding is what lets the lock be released and the state
written; aborting leaves a lock file on a machine nobody is watching and a half
-applied change nobody recorded.
Depth: [crates/nerpa-core/src/contract.rs](crates/nerpa-core/src/contract.rs).

**3. An invariant is not marked `enforced` without a test that attempts the
bypass its own entry names.**

Every entry in `docs/invariants.toml` writes down how the guarantee could fail.
`enforced` means somebody wrote the attempt and watched it be refused; `claimed`
means the code is believed to hold it and nobody has tried. Marking the second as
the first is the failure this project is most exposed to, because the registry is
what everything else trusts.

**4. A secret value is never written to state, to a plan, or to a terminal.**

State holds a reference and a keyed digest, because a digest answers the only
question state has, which is whether the value changed. Anything derived from a
secret is a secret, including a rendered file. A state file that never held a
credential has none to leak through a backup.

**5. The address grammar and the state format do not change silently.**

Both are frozen the moment a state file exists outside this machine. A change
means a version number that refuses to read what it does not understand, and a
written path from the old shape to the new. Reading an old file as though it were
a subset of the new one is how a tool destroys resources it can no longer name.

**6. A provider never answers "it is not there" for something it failed to read.**

Absence and failure are different answers. A permission error, an unreadable
byte, a broken link — each is a refusal, and refusing stops the resource being
planned. Reporting failure as absence makes the next plan propose creating what
already exists, and the apply overwrites it. This is the exact class of quiet
wrongness the project exists to remove, so it may not be reintroduced by a
provider.
Depth: [docs/decisions/0020-a-machine-that-cannot-be-reached.md](docs/decisions/0020-a-machine-that-cannot-be-reached.md).

**7. No `unwrap()` outside tests.**

`expect("...")` instead, and the string is the reason somebody will read in a
panic message at three in the morning. `unwrap` claims a thing cannot fail and
offers nobody grounds to believe it. Reaching for `unwrap_or_default()` to
satisfy this rule is worse than either: it turns an impossible case into a wrong
answer instead of a loud stop.

**8. The gate is not bypassed.**

Not with `--no-verify`, not by deleting a test that went red, not by an `allow`
without a `reason` saying why. A check that fails is either finding something or
is itself wrong, and both are fixed in the open. A gate anybody has learned to
step around has stopped being a gate, and everything the project claims on its
strength becomes a claim about nothing.

---

## Adding a rule here

A rule belongs in this file only if breaking it cannot be undone by the next
commit. Everything else is a coding standard, and coding standards live in
`docs/CODING_STANDARDS.md` where they can be argued about. This list is expected
to stay roughly this length; if it doubles, the extras were standards wearing a
red coat.
