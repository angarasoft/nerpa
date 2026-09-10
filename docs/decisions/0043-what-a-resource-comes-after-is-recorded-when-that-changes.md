# 0043. What a resource comes after is recorded when that changes, and the plan says so

- Status: accepted
- Date: 2026-09-11

## Context

Removal order comes from the edges state recorded, and state recorded them when
a resource was applied or, since
[0042](0042-a-resource-found-as-declared-is-adopted.md), adopted. A resource that
already matched and was recorded was planned as nothing, so an edge written in
later — an `after`, a reference, a reaction — never reached state, and taking a
file and its directory away removed them in one stage, in any order. State
written before edges were recorded in full has the same gap for every resource
whose dependency already existed when it was made.

The edges could be refreshed without a line in the plan: apply could rewrite
them for every declared resource, or the plan could carry them beside its stages
the way it carries what could not be reached. Either way the plan would stop
being everything apply does, and somebody approving it would be approving
changes to state they were not shown.

## Decision

A declared resource that matches its declaration and is recorded with edges other
than the ones it has now is planned as a reorder: shown with what it comes after,
sent nowhere, and recorded again from what was read.

## Consequences

Removal is ordered by the edges a resource has, however late they were written
in, and state from before edges were recorded in full is repaired by the first
plan that reads it.

The cost is plan lines about something nobody asked to see. Adding `after` to a
module used on two hundred machines shows two hundred reorders, and the first
plan after upgrading from a state that kept fewer edges shows one for every
resource affected, each saying that nothing is sent. A reorder lists every edge
the resource has, not only the new one, because the plan does not carry what
state remembered. Reorders are written to state once for the run, as adoptions
are, and for the same reason.
