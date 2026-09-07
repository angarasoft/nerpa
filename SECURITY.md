# Security

## Where this stands today

Nothing is released. There is no published crate, no binary anybody can
download, and no version in production that is not somebody's own build. The
repository is private while the early stages are in progress, and this policy is
written now so that it is already in force on the day it opens rather than
drafted in a hurry afterwards.

That also means there is one maintainer and no security team. What follows is
sized to that, and says so where it matters.

## Reporting

**Use GitHub's private vulnerability reporting** — the *Report a vulnerability*
button on the repository's **Security** tab. It opens a private thread and, if a
fix is needed, a private fork to write it in.

No address is published on purpose: an inbox in a public file collects a decade
of spam and stops working the day the maintainer changes.

**Do not open a public issue for a suspected vulnerability.** If the private
channel is unavailable to you, open a public issue containing only the words
"security report, need a private channel" and nothing else, and you will be
contacted.

## What is promised

| | |
| --- | --- |
| First reply | within **5 working days** |
| Assessment, or a reason it is taking longer | within **15 working days** |
| Coordinated disclosure | **90 days** from the first reply, or sooner if a fix ships sooner |

Modest on purpose. A single maintainer who promises a reply in 48 hours is
making a promise that breaks on the first holiday, and this project's own rules
forbid claiming more than can be demonstrated. If a report is credible and the
90 days are running out without a fix, say so and the disclosure date is agreed
between us rather than imposed by either side.

There is no bounty. Anyone who reports something real is credited by name in the
changelog and in the advisory, unless they would rather not be.

## What counts

The property this tool sells is that **the world ends up matching the plan, and
nothing else happens**. Anything that breaks that is in scope even when it does
not look like a classic vulnerability:

* A run that changes something the plan did not say it would, or fails to change
  something the plan said it would.
* A secret reaching the state file, a plan, a log or a terminal. State is
  supposed to hold a reference and a keyed digest and never a value.
* A resource address that escapes into a command line as an option, or into a
  path outside the tree it was confined to.
* A frame from the wire that allocates before its length has been bounded, or is
  read as a version it was not written by.
* A failure reported as an absence, so the next plan proposes creating something
  that is already there and the apply overwrites it.
* Once they exist: a plan hash that can be made to collide or be replayed against
  a different world, a grant that does not confine what it names, or a provider
  reaching a host or a credential its manifest did not declare.

## What is already known, and is not a report

`docs/invariants.toml` is the honest surface of this project. Every guarantee is
registered with the statement, **how it could be bypassed**, and whether anybody
has written a test that carries that bypass out. An entry marked `enforced` has
been attacked; one marked `claimed` is believed and undemonstrated, and its
`bypass` field says exactly where it is thin.

Five entries are `claimed` as this is written, and the ways around them are
published rather than hidden:

* `INV-PLAN-001` and `INV-SECRET-004` — an opaque value or a secret reaching a
  comparison through a function *parameter* is still answered silently. That
  includes a function which does nothing but hand the value back.
* `INV-FACT-001` — the same gap, for something read from a machine: a fact
  compared inside a function decides whether a resource exists. This entry said
  `enforced` while its own `bypass` field described the hole, which is worse
  than the hole.
* `INV-PLAN-003` — an attribute a provider does not report back never settles.
* `INV-STATE-004` — the state file's rename is atomic against a stopped process,
  not against a stopped machine.

Telling us about one of those is welcome as an ordinary issue and is not a
vulnerability report: it is written down, and a private channel buys nothing for
something already public. **Finding a fifth one is exactly what this section is
for** — a guarantee marked `enforced` that you can bypass is a real report, and
the most valuable kind, because the registry is what everything else here rests
on.

Dependency advisories are watched by `cargo xtask deny` on every run of the gate.
A report that a dependency has a published advisory is useful only if the gate is
not already catching it.

## Out of scope

Anything under a section of `docs/ARCHITECTURE.md` marked `planned` describes a
decision rather than code, and cannot be vulnerable yet. The markers are on every
heading for this reason.

Reports produced only by an automated scanner, with no explanation of how the
finding reaches a real run of this tool, will be closed. Not out of principle —
there is one person here, and reading them is the whole budget.

## Supported versions

None yet. When there are releases, the table of which ones still receive fixes
lives here, and `ROADMAP.md` says what a release means for this project: a tag at
a stage boundary, and versions stay `0.x` until stage 6, where a minor bump may
break anything.
