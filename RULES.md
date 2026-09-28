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

What may never be edited is the **reasoning**: the context that forced the
choice, the decision itself, and the costs it named. A **demonstrable error of
fact** — a miscount, a dead link, a renamed file — is corrected in place and
noted in the record, because superseding a whole record over a wrong number
would bury the reasoning under bookkeeping and teach everybody to ignore the
rule. If you cannot tell which of the two you are doing, it is reasoning.
Depth: [doc/adr/](doc/adr/).

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
Depth: `0020`.

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

## What holds each of them

A prohibition with nothing behind it is a wish, and this project's own thesis
says so. An independent review pointed out that this page was making eight
demands and naming no enforcer for any of them, which is the same failure it
diagnoses elsewhere. So the column exists now, and the dashes are the honest
answer rather than a gap in the table.

| Rule | Held by |
| --- | --- |
| 1 · a record is never rewritten | — *(a git-history check is the obvious next gate)* |
| 2 · the release profile unwinds | `cargo xtask lint` |
| 3 · `enforced` needs an attempt | `cargo xtask invariants`, **partly** — it checks that an adversarial test exists, and cannot check that the test attacks the bypass the entry names. The second half needs somebody who did not write the entry |
| 4 · no secret is written down | `INV-SECRET-001` … `004` and the tests they name |
| 5 · the address grammar and state format | **partly** — a state file from an unknown version is refused on read; nothing guards the grammar itself |
| 6 · failure is not absence | `INV-READ-001` |
| 7 · no `unwrap` outside tests | `cargo xtask lint`, and `clippy::unwrap_used` |
| 8 · the gate is not bypassed | — *(by construction: nothing can check that a check was not skipped)* |

Rule 3's half-answer is the interesting one, and it is not a defect to be fixed
by a cleverer gate. "A test exists" and "the test attacks the named bypass" are
different claims, and only the first is machine-checkable. Promotion from
`claimed` to `enforced` therefore wants a reader who did not write the entry —
which is how two entries came back from `enforced` on the day they left it.

## Adding a rule here

A rule belongs in this file only if breaking it cannot be undone by the next
commit. Everything else is a coding standard, and coding standards live in
`docs/CODING_STANDARDS.md` where they can be argued about. This list is expected
to stay roughly this length; if it doubles, the extras were standards wearing a
red coat.
