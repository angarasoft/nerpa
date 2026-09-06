# How documentation works here

Documentation rots when several kinds of it are kept in one place, because a
reader cannot tell which parts are still true and a writer cannot tell where a
change belongs. So each kind answers exactly one question and lives in exactly
one place.

| Question | Where | Checked by |
| --- | --- | --- |
| How do I use this thing? | rustdoc, in the code | `missing_docs`, `# Errors` and `# Panics` sections, no broken links |
| Why is it like this? | `docs/decisions/` | `cargo xtask docs` |
| What must never break? | `docs/invariants.toml` | `cargo xtask invariants` |
| What shape is it in now? | `docs/ARCHITECTURE.md` | `cargo xtask docs` |
| Where does this go? | `docs/MAP.md` | `cargo xtask docs` |
| What must never happen? | `RULES.md` | itself |
| How do I write a commit? | `docs/COMMITS.md` | `.githooks/commit-msg` |
| Something I noticed | `docs/WATCHLIST.md` | review |
| Something nobody has to answer yet | `docs/OPEN_QUESTIONS.md` | review |
| What may a plan cost? | `docs/budgets.toml` | `cargo xtask budget` |
| How do we write code here? | `docs/CODING_STANDARDS.md` | itself, mostly |
| How do we test it? | `docs/TESTING.md` | the invariant registry |
| What are we doing next, and when is it done? | `ROADMAP.md` | its own exit criteria |
| What changed? | `CHANGELOG.md` | review |
| How do I get started? | the documentation site | not built yet |

## The rule that keeps them apart

`ARCHITECTURE.md` describes **what is**. Decision records describe **why, and
when it was decided**. When the two disagree, the decisions are right and the
architecture document is stale — it is a summary, and summaries drift.

Three weights, and choosing the wrong one is the mistake this project actually
makes. An **observation** costs a row in the watchlist and commits nothing. A
**question** carries the stage by which it must be answered. A **decision** is an
obligation that narrows what may be built later, and undoing it costs a second
record. Writing a decision when an observation would do is how twenty-nine
records came to exist before the program reached a second machine — see
[0030](decisions/0030-claim-less-and-attack-what-is-claimed.md).

A decision record is never edited after it is accepted. It is superseded, and
the old one stays with a pointer forward. `0003` chose lazy planning and `0006`
undid it; keeping only the second would leave the next person free to propose the
first again.

## What a change has to bring with it

- Changes behaviour → an entry in `CHANGELOG.md`.
- Changes a decision, or makes one → a record in `docs/decisions/`.
- Adds a guarantee the tool must hold → an entry in `docs/invariants.toml` with
  the `bypass` it could suffer, an `// ENFORCES:` anchor, and a test the entry
  names. It is `claimed` until an `adversarial` test carries that bypass out.
- Builds something the architecture described as planned → the section's marker
  in `docs/ARCHITECTURE.md`.
- Adds a public item → rustdoc on it, with `# Errors` if it returns `Result` and
  `# Panics` if it can.

None of these are review conventions to be remembered. The first is, the rest
fail the gate.

## Language

**English in the repository.** All of it — code, comments, records, standards,
changelog. This is where contributors read.

Russian belongs on the `.ru` site, and it is a **translation of the repository,
never an independently written version**. Two versions written separately drift
within a quarter, and after that one of them is lying to somebody without anyone
noticing.

## The site

Not built yet, and deliberately last. When it is: **Sphinx**, assembled from
sources that already exist and are already checked —
[0036](decisions/0036-assemble-the-book-and-let-prose-explain-not-promise.md).

* **Order** comes from `ARCHITECTURE.md`. Its sections are a sequence somebody
  curated and a gate keeps honest; the book does not need a second one.
* **Explanation** comes from module `//!` documentation, read at build time.
* **Promises** come from `docs/invariants.toml`, and from nowhere else. Prose may
  explain anything and may assert a guarantee only by naming an invariant, so the
  book cannot promise more than the registry — which is itself checked against
  the code in both directions.
* **Examples** are executed during the build rather than pasted, because
  evaluation is hermetic and a second run has to be empty.
* **Russian** is a `gettext` catalogue keyed to the English source. An
  independently written translation is not expressible, which is what makes the
  rule below a rule rather than a hope.

Every page says whether it was derived or written. What cannot be derived is
known in advance: the reading order, the comparison with Terraform and Ansible,
and the reason anybody should care — judgements about the world outside this
repository, which nothing in the source knows.

Reference material that belongs to the code — the address grammar, the resource
kinds a provider offers — is generated, not retyped. Anything retyped is a second
copy that will disagree with the first.
