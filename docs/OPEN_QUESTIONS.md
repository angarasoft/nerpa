# Open questions

Questions whose answers are not due yet, and the moment by which they are.

A question here is not an admission that something was missed. It is the opposite
— it is a decision deliberately *not* taken, held open until the thing that will
decide it has happened. The alternative is what already happened once in this
repository: an answer written before the experience, and a record that had to be
superseded.

Every entry names the stage by which it has to be answered, and what makes that
the deadline. A question with no deadline is a
[watchlist](WATCHLIST.md) entry; a question whose deadline has passed is a
decision somebody is making by not making it.

An answered question is moved to a decision record and struck from this file with
a pointer to the record.

## Open

### OQ-005 — How a plan is hashed, and what the hash covers

**Answer by:** before the executor sends its first batch — stage 1.
**Why then:** [0033](decisions/0033-process-is-a-providers-word-not-an-authors.md)
makes `Operation::Process` general enough to run anything, and what limits that
authority is the plan the batch came from.
[0034](decisions/0034-a-plan-carries-its-premise.md) binds a batch to the premise
it was computed from. Both reason from a plan hash that is named in the
architecture, in the testing document and in four rustdoc comments, and
implemented nowhere. Whichever way it is eventually computed, some of that prose
will turn out to have been about something else. It was `W-003` on the watchlist
until the executor gave it a deadline.

## Answered

| Question | Answered by | Date |
| --- | --- | --- |
| OQ-001 — threads or async | [0031](decisions/0031-parallelism-is-threads-and-providers-are-sync.md), and `INV-PROVIDER-001` | 2026-09-06 |
| OQ-002 — how bytes are represented | `Value::Bytes` and `INV-VALUE-001` — base64 under a reserved key in a readable format, a real byte string on the wire | 2026-09-06 |
| OQ-003 — whether a directory owns what is inside it | It owns its own existence only. `remove_dir`, never `remove_dir_all`; a directory with something in it refuses. `INV-DELETE-001` | 2026-09-06 |
| OQ-004 — state once per run or once per change | Once per change. `apply` takes somewhere to write to and uses it after every one. `INV-STATE-005` | 2026-09-06 |

Three of those four point at an invariant rather than at a decision record, and
that is deliberate. [0030](decisions/0030-claim-less-and-attack-what-is-claimed.md)
asks for fewer records, not more; an answer whose whole reasoning fits in a
registry entry and a commit message does not need a page of its own. A record is
for a choice somebody could reasonably have made differently.

## Adding a question

Two fields and no more: the date or stage by which an answer is needed, and what
makes that the deadline. A question without a stated deadline drifts, and the
project ends up answering it accidentally in an implementation.
