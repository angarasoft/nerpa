# RFCs

A proposal big enough that a mistake in it would be expensive to find in code:
several decisions at once, a protocol change, a contract that strangers write
against. It is written in full before any of it is built, reviewed on purpose,
and then turned into the decision records and architecture sections it implies.

A decision record says why one thing was decided. An RFC is where a change that
needs several of those is worked out, measured and attacked first, so the records
that come out of it can be short: the arguing already happened, and it is kept.

## Writing one

Take the next free number. Put what was measured before what is proposed. An RFC
that reasons from numbers nobody took is a guess with headings.

Two more rules. Every section that changes an invariant names it. Every risk says
whether it is solved, bounded or accepted.

## Status

| Status | Meaning |
| --- | --- |
| `draft` | Being written. |
| `in review` | Reviewers are working through it, and their findings go in its review log. |
| `accepted` | Settled. Its decisions are recorded in `docs/decisions/`, and the RFC points to them. |
| `withdrawn` | Not pursued, with the reason. Never deleted. |

## Review

Each reviewer takes one angle and says which. A finding records:

- what is claimed;
- what is wrong with it;
- the evidence;
- a proposed resolution.

Each finding is then checked against the code or a measurement before it is
answered.

The review log at the end of the RFC keeps every finding and what became of it,
including the ones rejected and why.

An accepted RFC is not edited. What changes after acceptance is a decision record,
like any other.
