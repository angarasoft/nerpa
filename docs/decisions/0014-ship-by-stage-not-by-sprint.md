# 0014. Ship by stage, with an executable exit criterion

- Status: accepted
- Date: 2026-09-04

## Context

Sprints are a coordination mechanism for teams. This project has one maintainer
who is also running a second product, so a two-week cadence would buy ceremony
and no coordination. The failure mode to guard against is different: work that
drifts for a month on a branch, and a stage declared finished because it feels
finished.

## Decision

The roadmap stages are the plan, and each has one written exit criterion that can
be executed rather than judged. A stage ends when its criterion passes on real
hardware, not when its description feels satisfied.

Between those, work in slices that end with a green `cargo xtask ci` and a
commit. A branch that lives longer than a couple of days is a slice that was cut
too large.

Releases are tags at stage boundaries, versioned `0.x` until stage 5, where a
minor bump may break anything. `CHANGELOG.md` is written as the work happens, not
reconstructed from the log. Once anything is published, `cargo-semver-checks`
joins the gate.

## Consequences

Progress is claimed against a command that either passes or does not, which is
the only honest report a solo project can give itself. There is nothing to
schedule, attend or estimate.

The cost is that a stage with a badly written criterion becomes either a
formality or an unreachable bar, so the criteria are reviewed as carefully as the
code. And the plan hash is now a release concern: a change that alters the hash
for identical input invalidates signatures in the field, and has to be called out
in the changelog whether or not the public API moved.
