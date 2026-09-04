# 0006. Plan in one pass; let stages belong to execution

- Status: accepted
- Date: 2026-09-04

## Context

`0003` chose lazy planning because resources in the `target` domain seemed
impossible to plan before their nodes existed. Two things that were decided
afterwards removed that reason.

Starlark (`0005`) returns unresolved values as opaque handles that cannot be
branched or iterated over, so the set of resources is always known after
evaluation even when individual values are not. And for a node this run is
creating, observed state is not unknown but empty — the node is new, so the diff
is "create everything", which is computable before anything happens. A node that
already exists is reachable and can simply be read.

Two distinct things had been conflated: stages of execution, which the graph
forces, and stages of planning, which were a choice.

## Decision

Planning is a single pass over the whole graph. Unresolved values are marked and
carry a reference to the resource that will supply them, and they never determine
which resources exist. Stages order execution only.

## Consequences

The bounds machinery from `0003` is deleted outright. The plan hash can include a
digest of the observed state it was computed against, so a signature cannot be
replayed against a world that has changed since — which lazy planning could not
express at all. An agent sees the whole blast radius at once.

Three costs, and they are not small. Every existing target must be readable at
plan time, so fingerprints and pushed reports become load-bearing rather than an
optimisation. Plans grow with the fleet, so aggregating identical changes in the
output is mandatory rather than a nicety. And a resource whose *existence*
depends on a fact discovered on a node that does not exist yet cannot be
expressed at all — that case is refused rather than approximated.
