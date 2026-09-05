# Commits

```text
[TYPE] scope: subject

The body, after a blank line, saying why.
```

The scope is optional. Everything else is checked by `.githooks/commit-msg`, so
this page is a description of a gate rather than an appeal to anybody's memory.

## Install the hook

```console
$ cargo xtask hooks
```

Once per clone. Hooks are not cloned, so a working copy that has not run this is
not being checked — which is the one thing the hook itself cannot do anything
about.

## The types

Closed list. An open one is a list of synonyms waiting to happen, and this
repository proved that at twenty-five commits: `[DOC]` appeared twice and
`[docs]` three times, for the same act, before anybody noticed.

| Type | What it marks |
| --- | --- |
| `FEAT` | Something the tool can do that it could not do before |
| `FIX` | Behaviour that was wrong and is not now |
| `PERF` | The same behaviour, measurably cheaper |
| `CHANGE` | Different on purpose: a policy, a default, a name, a boundary |
| `DOCS` | Writing only. No behaviour moves |
| `TEST` | Tests only, including the fixtures that prove a gate can fail |
| `CI` | The gate itself, the pipelines, the hooks |
| `CHORE` | Dependencies, versions, housekeeping nobody looks up later |

`CHANGE` is doing work here that `refactor` does elsewhere, and it is kept
because it says something `refactor` does not: the behaviour is different, and it
is different because somebody decided so. A commit that changes a default, a
name or a boundary is not a refactor and pretending otherwise loses the one fact
a reader wants.

If a change wants two types it is two commits.

## What is refused

* Anything but `[TYPE] ` at the start, with exactly one space after the bracket.
* A type outside the list. A known synonym is told which token to use instead —
  `[DOC]` is answered with `[DOCS]`, not with the whole list — and so is the
  right word in the wrong case.
* An empty subject, or one ending in a full stop.
* A body that runs straight on from the subject. There is a blank line between
  them, because every tool that shows a log assumes it.
* A subject over 90 characters.

## What is only mentioned

A subject over 72 characters prints a note and the commit proceeds.

Six of the first twenty-five subjects here are over 72 and the longest is 84.
Refusing at 72 would reject a quarter of the practice it is meant to keep, and a
gate that rejects established practice is a wall — people climb walls, or turn
them off. The hard limit sits above everything written so far, which caps the
drift without relitigating the past. Tighten it by bringing the practice down
first, never by turning the wall on.

Recheck the claim rather than trusting this paragraph:

```console
$ git log --format='%s' | awk 'length>72' | wc -l
```

## What is not checked, deliberately

**The mood of the subject.** Imperative is the convention and it is not
machine-checkable without false positives, so it stays a matter of review.

**Whether the body exists.** Most commits do not need one. The ones that do are
the ones that were hard, and no rule makes somebody explain a decision they did
not know they were making.

**Merges, reverts and `fixup!`.** Git writes those, or eats them.

## The body

Where the reasoning goes, and the only part of a commit anybody reads a year
later. Say what was wrong, not what you did — the diff already says what you did.

The best one in this history is `[FIX] os: a file it may not read is not a file
that is not there`, which spends four paragraphs on why an `Option` could not
carry the answer the function needed to give. That is what the field is for.
