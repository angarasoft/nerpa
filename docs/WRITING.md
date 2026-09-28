# Writing

How prose is written here, and where each kind of it goes.

This is a separate file rather than a section of something else because it is
expected to grow: the moment module documentation became chapters of a book, the
rules stopped being about comments and started being about a publication. Where
things go is [MAP.md](MAP.md); which kind answers which question is
[DOCUMENTATION.md](DOCUMENTATION.md); this is *how it is written*.

## The one rule everything else serves

**Prose explains. The registry promises.**

A chapter may explain anything — why one planning pass is enough, why a contract
failure has to stop the process, what a slot is for. It may not assert a
guarantee except by naming an invariant, which renders its statement, how it
could be bypassed, and whether anybody has attacked it.

The reason is not tidiness. `doc/invariant/` is checked against the code in
both directions and a claim in it has to survive an adversarial test before it
may say `enforced`. A sentence in a document is checked by nobody. Letting prose
promise things puts the weakest surface in the project in front of the reader,
saying the same kind of thing as the strongest one, in the same voice.

This rule is held by review, not by a gate — see the table at the end, and the
paragraph explaining why a gate would be worse than nothing here.

## Voice

Impersonal. Not "we decided", not "our engine" — the project is not a character
in its own documentation, and the reader is not being addressed by a committee.
This was already the convention before it was written down: one first-person
plural in seven hundred lines of module documentation, and it was inside a quoted
phrase.

Present tense for what exists. Something settled and unbuilt is described in the
present tense too — that is how a contract reads — and the section's marker
carries the difference. Never write "will" to mean "does".

Say what breaks without the thing. A sentence that survives six months is the one
that stops somebody simplifying a decision back out. A sentence that restates the
code is worse than no sentence, because it rots without anybody noticing.

Name the source of a number. "Four times the resources used to be twenty-two
times the work" is followed, in the file that says it, by the measurements and
the command that reproduces them. A figure with no provenance is a figure nobody
can check and everybody repeats.

## Module documentation is a chapter

A module joins a chapter of the book by naming the decision record that settled
its part of the architecture:

```rust
//! Turning desired and observed state into a plan, in one pass.
//!
//! Expands: 0006
//!
//! # Why one pass is enough
//!
//! ...
```

`cargo xtask book` refuses a record that does not exist or has no section in
`ARCHITECTURE.md`, in both directions. The record number is the join key because
rule 1 makes it immutable; a section heading is prose somebody may improve.

Most modules do not name one, and should not. Plumbing has nothing to say to a
reader who does not already have the source open.

What a chapter-module's header owes:

* **A first line that is one sentence**, ending in a full stop. It becomes the
  summary everywhere the module is listed.
* **What the piece is for, before what it contains.** A list of items is a table
  of contents; a chapter opens with why anybody would read it.
* **A diagram where the shape is the point.** In a `text` block, drawn with box
  characters, as `nerpa-core/src/lib.rs` does.
* **Sections under `#`**, which the book pushes down two levels to sit under the
  chapter.

Rustdoc link syntax — ``[`Address`]`` — is fine and expected: it works in
`cargo doc`, and the book renders it as plain code until the API reference is
shipped beside it.

## Decision records

Immutable once accepted. A demonstrable error of fact is corrected in place and
noted in the record; the reasoning is superseded, never rewritten. That is rule 1
and it outranks everything on this page — including the voice rules, which is why
two records still say "we" and will go on saying it.

Short. `doc/adr/` puts it best: if it does not fit on a page, it
is probably two decisions. Name the cost, or the record is advocacy.

## Language

English in the repository — all of it. Russian belongs on the `.ru` site and is a
**translation** produced through `gettext`, never an independently written
version. That is not a promise to be careful: the catalogue is keyed to the
English source strings, so an English edit marks its entry stale and an
independently written Russian page is not expressible.

## What is held by what

| Rule | Held by |
| --- | --- |
| Every module opens with `//!` | `cargo xtask lint` |
| `Expands:` names a record with a section | `cargo xtask book`, both directions |
| An `INV-` named in a header is registered | `cargo xtask invariants` |
| No first person in module documentation | `cargo xtask lint` |
| No `TODO`, `TBD` or `FIXME` in a header | `cargo xtask lint` |
| Records are numbered, complete, and name a cost | `cargo xtask docs` |
| Every link resolves | `cargo xtask docs`, and `sphinx-build -W` |
| **Prose explains and does not promise** | — *review* |
| **Explains why rather than what** | — *review* |
| **A number names where it came from** | — *review* |
| **Present tense means what exists** | — *review* |

The four dashes are deliberate and worth defending. Each could be approximated by
a gate — banning "guarantees", "always", "never" — and each approximation would
be wrong more often than right: this repository uses *never* correctly on almost
every page (*"never stored beside it"*, *"never turned into work"*). A gate that
fires on good prose teaches people to work around it, and then it holds nothing
at all while looking as though it holds something. These four are review rules,
and saying so is better than pretending otherwise.

## Adding to this file

A rule belongs here if it is about how a sentence is written. Where a document
goes is `MAP.md`; what it must contain to be a decision record is
`doc/adr/`; how the code is written is `CODING_STANDARDS.md`.
If a rule cannot be stated in a sentence and defended in two, it is a preference
rather than a rule.
