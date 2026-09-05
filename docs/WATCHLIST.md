# Watchlist

Observations that are not yet decisions.

Something noticed — a gap, an idea, a thing that will probably bite — goes here
first. It costs one row. It commits the project to nothing, which is the point:
a decision record is an obligation, and 0003 had to be reversed by 0006 because
the obligation was taken before the experience that would have informed it.

An entry leaves this file in one of three ways, and always by being **moved**,
never deleted: into a decision record, into
[OPEN_QUESTIONS.md](OPEN_QUESTIONS.md) when it turns out to be a question with a
due date, or into the closed list below when it stops mattering. Every departure
says what became of it, so the file is also the record of what was considered and
put down.

Only live entries sit in the table. If it has been escalated or closed, it is in
the second table.

## Watching

| ID | Observation | Why it might matter | Added |
| --- | --- | --- | --- |
| W-001 | One-shot commands — `ansible all -a uptime` — are marked not scheduled | The roadmap already concedes this "matters more for adoption than its size suggests". Nothing above it is blocked by it, which is exactly how a small thing stays undone for two years | 2026-09-05 |
| W-002 | Reading a resource without taking it under management has no home | Terraform's data sources. Not needed to reach a fleet, needed the first time somebody has to place a machine in a subnet they did not create | 2026-09-05 |
| W-005 | The design depends on providers written by people outside this repository, and the repository opens at stage 6 | Five stages are built without a single outside author, and the interface they will write against is settled in that time with no feedback from anybody who has to use it | 2026-09-05 |
| W-006 | A file's contents travel through the plan, the state file and the wire as one string | Fine for `listen 80;`. A plan that prints a megabyte of certificate is a plan nobody reads, and a state file that carries it is a state file nobody opens with `jq` | 2026-09-05 |
| W-008 | The executor binary has to reach the machine, matching its architecture and its libc | Ansible sidesteps this because Python is already there. A static binary is five to fifteen megabytes; pushing it to two hundred machines every run is not an option, so there is a cache keyed by version, a version handshake and a cleanup policy — none of them costed, and all of them part of the protocol rather than of the implementation | 2026-09-06 |
| W-009 | A value that only exists once an operation has run costs a round trip per stage boundary | The uid `useradd` mints is not knowable at plan time. Typical depth is two to four, so it is bounded; a deep chain eats the advantage that central planning bought. The way out — teaching the executor to resolve references inside a batch — turns it into an interpreter, which is the escalation to watch for | 2026-09-06 |
| W-010 | The control machine holds every target distribution's semantics | `emerge` against `apt`, version comparison, USE flags: all decided here, from facts, rather than by looking. Ansible puts that knowledge on the machine it describes. Every provider carries a distribution matrix, and being wrong about one is discovered remotely | 2026-09-06 |
| W-007 | A provider silently accepts an attribute it does not own | Measured: a file declared with an attribute the `os` provider does not know applies cleanly and then re-plans for ever, because nothing reports the attribute back and nothing ever settles it. Either a provider refuses what it cannot own, or the planner learns that an attribute absent from a reading is not the same as one that differs. The choice belongs with the provider trait, which outside authors will implement | 2026-09-05 |

## Closed

| ID | Observation | What became of it | Closed |
| --- | --- | --- | --- |
| W-003 | The plan hash is named everywhere and implemented nowhere | Escalated to `OQ-005`. The executor gave it a deadline: 0033 makes a batch able to run anything and 0034 binds it to a premise, and both reason from the hash | 2026-09-06 |
| W-004 | Refusing an unresolved value is syntactic, so every new way of laundering one needs a new pattern | Became an analysis rather than a pattern: opacity now spreads through assignments, containers, indexing, iteration, comprehensions and function returns. What is left — a function *parameter* — is named in `INV-PLAN-001`'s bypass and has a test that goes that way | 2026-09-06 |

## Adding an entry

Give it the next number, say what was seen and why it might matter, and stop.
An entry that already knows its own answer is not an observation; it is a
decision, and it belongs in `docs/decisions/`. An entry with a date by which
somebody has to answer is a question, and it belongs in `OPEN_QUESTIONS.md`.
