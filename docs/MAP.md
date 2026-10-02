# The map

Where each kind of knowledge lives, and what stops a change to the code from
quietly leaving the writing behind.

Two tables and nothing else. The first says where to put something; the second
says what has to move when something else does. Both are short on purpose: a map
that tries to list every file is a second copy of the tree, and it is wrong
within a week.

## One kind of knowledge, one home

| Kind | Home |
| --- | --- |
| Why a decision was taken, and when | `doc/adr/` — the registry, written as code, immutable once accepted, superseded rather than edited |
| A change worked out, measured and attacked before its decisions are taken | `doc/proposal/` — reviewed on purpose, with the findings kept in the document, then turned into records |
| What the system is, as contracts | `docs/ARCHITECTURE.md` — every section names its record and says whether it is built |
| What must never break, and how it could | `doc/invariant/` — the registry, checked against the code both ways |
| What is being built, in what order | `ROADMAP.md` — stages with exit criteria that can be executed |
| An observation not yet worth a decision | `docs/WATCHLIST.md` |
| A question whose answer is not due yet | `docs/OPEN_QUESTIONS.md` |
| Rules whose breach breaks the repository | `RULES.md` |
| How a commit is written | `docs/COMMITS.md` |
| How prose is written | `docs/WRITING.md` |
| How to report a vulnerability, and what counts as one | `SECURITY.md` |
| How people are expected to behave | `CODE_OF_CONDUCT.md` |
| What a report from outside must contain | `.github/ISSUE_TEMPLATE/` |
| The book, and how it is assembled | `docs/book/` — chapters are generated, never checked in |
| What an agent working here is told | `AGENTS.md` — generated from the registry by `cargo dacc emit all`, checked by the gate, never edited by hand |
| How to work in the test suite | `docs/TESTING.md` |
| What the gate enforces, and what review does | `docs/CODING_STANDARDS.md` |
| Which kind of documentation answers which question | `docs/DOCUMENTATION.md` |
| What a released version changed | `CHANGELOG.md` |
| What a plan is allowed to cost | `docs/budgets.toml` |
| How the code itself works | Rustdoc, beside the code — never a Markdown file describing a module |
| The long-form body a proposal is read as | `docs/rfcs/` — embedded into the registry as the proposal's body; its typed specification is `doc/rfc/` |
| How the repository opens to people outside it | `docs/opening-the-repository.md` |
| What a live run is shown with | `live/` — examples, never the source of truth |

One kind of knowledge has one canonical home. Everything else links to it. A
document that copies another is a document that will disagree with it, and the
reader has no way to tell which half is stale.

Getting around the registry itself is `cargo dacc map` and `cargo dacc where`
from the dacc release that carries the access layer — never `grep` over
`doc/`.

Routes here name directories and stable files, never a stage number. A path with
`stage 1` written into it is wrong the day stage 1 closes, and it goes on
pointing somewhere plausible.

## What has to move together

An event on the left without its right-hand column is drift: the code says one
thing and the writing says another, and nobody finds out until somebody trusts
the writing.

| When this changes | This changes with it | Held by |
| --- | --- | --- |
| A guarantee is added, or one stops holding | `doc/invariant/`, a `#[doc_anchor]`, the tests it names | `cargo xtask invariants` |
| A decision is taken or reversed | A new record in `doc/adr/`, and the superseded one points forward | the gate, `cargo xtask ci` |
| Something in the architecture is built, or turns out not to be | The section's status in `docs/ARCHITECTURE.md` | `cargo xtask docs` |
| A file named in this map moves | This map | `cargo xtask docs` |
| The planner's shape changes | `docs/budgets.toml`, if the measured cost moved | `cargo xtask budget` (nightly) |
| A stage's exit criterion is met | `ROADMAP.md`, `CHANGELOG.md` | — |
| A public interface changes | `CHANGELOG.md` | — |
| A commit is written | nothing else — the subject carries its own shape | `.githooks/commit-msg` |
| A module becomes a chapter, or stops being one | its `//! Expands: NNNN`, and a section in `ARCHITECTURE.md` | `cargo xtask book` |
| An invariant is promoted or demoted | `SECURITY.md`, if it names it among what is already known | `cargo xtask invariants` |
| An observation is escalated to a decision | `docs/WATCHLIST.md`, and the record that took it | — |

The dashes are not omissions. They mark obligations that today rest on whoever
is writing, and they are the list of gates worth building next — in that order,
because a rule with nobody to enforce it is a rule that gets skipped and nobody
notices. That is not a suspicion; it is the reason every other row in the column
has an entry.

## Adding to this map

A new row belongs here only if it names a **kind** of knowledge, not a file. If
the answer to "where does this go" is already covered by a row above, the answer
is that row, and the map stays the size it is.
