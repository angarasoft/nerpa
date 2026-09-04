# 0003. Plan lazily, in stages

- Status: superseded by 0006
- Date: 2026-09-04

## Context

Resources in the `target` domain (`0001`) cannot be read until the node they
live on exists, and that node may be created by the same run. A single planning
pass therefore appeared impossible: the diff for a package cannot be computed
before the machine it belongs to is there.

## Decision

Plan one stage at a time. Apply a stage, learn the values it produced, then plan
the next. Each deferred stage carries a statically computed upper bound on what
it may do, so that an approval can be given before the whole plan is known.

## Consequences

Planning works in the presence of resources that do not yet exist.

The cost is a whole subsystem — the bounds machinery — that exists solely so
that something not yet computed can be signed, plus a plan output that is
partly honest and partly an estimate.

## Why this was reversed

`0005` chose Starlark, where an unresolved value is an opaque handle that cannot
be branched or iterated over. That makes the *set* of resources always known
after evaluation even when individual values are not, which removes the reason
this decision existed. See `0006`.
