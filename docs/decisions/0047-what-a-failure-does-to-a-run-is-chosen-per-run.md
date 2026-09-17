# 0047. What a failure does to the rest of a run is chosen per run

- Status: accepted
- Date: 2026-09-17

## Context

A serial run stopped at the first failure, and nobody chose that: it was what one
loop does. Over a fleet the two wishes are both reasonable. A person rolling out a
change wants the first failure to stop everything. A person repairing a fleet
wants everything that can be done to be done, and the failures named.

## Decision

Two policies, chosen per run and printed in the plan's header. Under `stop`, the
first failure anywhere sets the stop flag and no change begins after it. Under
`continue`, a change that failed is refused, and so is everything that waits for
it, transitively; everything else goes on, every refusal is named with its cause,
and the run exits with a failure.

What a change waits for is in the plan and part of its number: the edges state
recorded, and the edges that only order this run, which are shown but never
recorded. `INV-APPLY-001` holds over every path: a change begins only once every
change it waits for has its outcome durable — a record, a removal's forget, a
replacement's create — and its debts in other shards are written; under `stop`,
none begins after the flag is set.

Some things are always a stop, whatever the policy: a state write that fails, a
contract violated by the far end, a signal.

## Consequences

`continue` leaves a fleet part changed on purpose; it is named in every refusal,
in the plan's header and in the exit status, and it is not the default. One
refusal check in the recorder is consulted by every path, which is one more thing
on the path of every change. A reacting resource is refused when its cause fails,
and its debt stays owed, so a later run restarts it once: the redundant restart
0026 accepts. The cost is that a failed run now has two shapes for a
person to read, and that `continue` can leave a fleet no single sentence
describes.
