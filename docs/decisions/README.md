# Decisions

One file per decision, numbered, and **immutable once accepted**. A decision that
turns out wrong is not edited — a new one supersedes it, and the old one stays
where it is with a pointer forward.

That rule is the whole value. Half the decisions here were reversals, and the
reversal is the useful part: `0003` chose lazy planning for reasons that were
correct at the time, and `0006` undid it because a later choice removed the
reason. Keeping only the answer would leave the next person — or the next agent —
free to propose lazy planning again, and nobody would remember why not.

## Writing one

Copy [`0000-template.md`](0000-template.md), take the next free number, fill in
every heading. Short is good: if it does not fit on a page, it is probably two
decisions.

**Context** is what was true before, including the constraint that forced a
choice. **Decision** is one sentence in the active voice. **Consequences** must
name what this costs, not only what it buys — a decision record with no downside
listed is marketing, and the next person will not trust the rest of it.

## Status

| Status | Meaning |
| --- | --- |
| `accepted` | In force. |
| `superseded by NNNN` | Was in force, is not now. Never deleted. |
| `proposed` | Written down, not settled. |

`cargo xtask docs` checks the numbering, the headings and that every supersession
points both ways.
