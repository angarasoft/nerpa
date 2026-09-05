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

### OQ-001 — Threads or async, and what that does to the provider trait

**Answer by:** before stage 3 begins.
**Why then:** stage 3 says machines are visited in parallel. `Providers` holds
`Box<dyn Provider>` with neither `Send` nor `Sync`, and the one provider that
exists keeps a `RefCell`. Making the trait parallel changes the interface that
0004 says outside authors will implement. There are no outside providers today,
so the change is free; after stage 6 it is a breaking change to somebody else's
crate.

### OQ-002 — How bytes are represented

**Answer by:** before a state file written by this code exists on a machine that
is not the author's.
**Why then:** `Value` is `#[non_exhaustive]` and grows cheaply, but the state
format and the wire protocol are frozen by rule 5 the moment either leaves. A
certificate, a keytab or a file that is not UTF-8 has nowhere to go today, and
the manifest already justifies CBOR on the wire by its ability to say "these are
bytes" — an ability nothing currently uses.

### OQ-003 — Whether a directory owns what is inside it

**Answer by:** before the executor gains anything that removes a path on a real
machine — stage 1.
**Why then:** `delete` on a directory currently removes its contents recursively.
Nothing put them there, and nothing recorded them. Either a directory resource
owns its subtree and that is stated, or it owns only its own existence and
removal refuses when it is not empty. Both are defensible; doing neither
deliberately is not, and the wrong choice is discovered on somebody's machine.

### OQ-004 — Whether state is written once per run or once per change

**Answer by:** before the executor reaches a machine that is not this one —
stage 1.
**Why then:** state is written after `apply` returns. Contracts panic by design,
and a panic unwinds past the only call that would record what was already made.
Locally this loses a file nobody minds. Across a fleet it loses the record of
every resource created before the stop, and the next run proposes making them
again.

## Answered

| Question | Answered by | Date |
| --- | --- | --- |
| — | — | — |

## Adding a question

Two fields and no more: the date or stage by which an answer is needed, and what
makes that the deadline. A question without a stated deadline drifts, and the
project ends up answering it accidentally in an implementation.
