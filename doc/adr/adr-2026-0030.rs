use crate::taxonomy::Subsystem;
use slipway_core::{nonempty_str, taxon};
use slipway_knowledge::{Breaking, DocStatus};

slipway_knowledge::adr!(
    title: "Claim less, and attack what is claimed",
    status: DocStatus::Active,
    subsystems: &[taxon!(Subsystem, Gate)],
    context: r"- Corrected: 2026-09-06 — a miscount of the architecture markers, and two
  invariants named here as promoted that had to go back. Facts only; the
  reasoning is as it was written.
- Corrected: 2026-09-06 — the codebase this rule was learned in was named. It
  belongs to somebody who did not agree to appear here, and the argument never
  rested on which codebase it was.
## Context

Two habits had grown up together, and each made the other harder to see.

**Every thought became a decision.** Twenty-nine records were accepted before the
program had touched a machine it was not running on. A record is an obligation:
it narrows what may be built later, and undoing one costs a second record and an
explanation. `0003` had already been reversed by `0006`, which is the system
working — but the reason it needed reversing was that the decision was taken
before the experience that would have informed it. There was nowhere to put an
observation, a doubt or a question, so each arrived as an answer.

**Every belief became a proof.** `docs/invariants.toml` had two statuses,
`planned` and `enforced`, and `enforced` meant only that some code and some tests
existed. Nothing asked whether the tests attacked the guarantee or merely
exercised it. The registry, the `// ENFORCES:` anchors and a green gate together
produced a strong impression of demonstration, and four entries did not survive
being attacked:

* `INV-PLAN-001` — an unresolved value never decides which resources exist. The
  refusal is a pass over the syntax tree matching a bare attribute access beside
  `==`. A handle stored in a list or a dict and read back out is a different
  shape, Starlark answers the comparison without consulting the value, and a
  resource is created on the strength of something that does not exist.
* `INV-SECRET-004` — the same hole, reached the same way, one week old.
* `INV-PLAN-003` — a second run is empty. An attribute the provider does not
  report back never settles, so the run is never empty and the tool proposes the
  same update for ever.
* `INV-STATE-004` — state is written whole. The rename is atomic against a
  stopped process; nothing calls `sync_all`, so it is not atomic against a
  stopped machine.

None of these was found by the gate, because the gate checks that the links
between the registry, the code and the tests have not rotted. It never asked
whether the tests were about the guarantee.

A codebase fifty times the size had already met both problems and written down
the rule that came out of it: an obligation with no gate behind it is skipped,
and nobody notices. Its answer is proportionate to its size. The rule underneath
it is not.
",
    decision: r"
An invariant is `claimed` until somebody writes down how it could be bypassed and
a test carries the bypass out; and an observation, a question and a decision are
three different documents with three different weights.
",
    trade_offs: &[r#"
Every built invariant now records `bypass` — what an attempt to violate it would
actually look like — and `enforced` additionally requires `adversarial` tests
that perform it. Where the types make a violation impossible to express,
`bypass = "unrepresentable"` says so and no test is asked for. Each of the four
above moved to `claimed` with its reproduction recorded, and `cargo xtask
invariants` refuses the promotion of a claim back to a proof without the test,
which is rule 3.

Two of them left `claimed` the same afternoon, closed by tests named after the
routes their entries described — through a dictionary, through a list. The
registry named a hole in terms somebody could act on, and somebody did.

Then both came back. An independent review found the same comparison still
answering silently when the value arrives through a function *parameter* and the
other side is a constant, which is what those entries' own `bypass` field says.
`enforced` was wrong for them, and this record would have gone on saying
otherwise. That is the limit of the mechanism, and it is worth stating plainly:
a gate can check that an adversarial test exists, and cannot check that the test
attacks the bypass the entry describes. Those are different claims and only the
first is machine-checkable. The second needs somebody who did not write the
entry — which is how this correction came to exist.

`docs/ARCHITECTURE.md` marks each section `built`, `partly built` or `planned`.
The document is written in the present tense because that is how a contract
reads, and twelve of its nineteen sections describe something that is not all
there: seven are partly built and five are not built at all.

`docs/WATCHLIST.md` takes an observation for the price of one row, and
`docs/OPEN_QUESTIONS.md` takes a question with the stage by which it must be
answered. Neither commits the project to anything, which is the point: the cheap
place to put an unripe thought is what stops it arriving as a decision.

`docs/MAP.md` says which kind of knowledge lives where, and what has to move when
something else does. Its enforcement column carries dashes where an obligation
rests only on the author, and those dashes are the list of gates worth building
next.

The costs are real. Three statuses are harder to read than two, and the honest
count of what is proven drops the day this lands — the number goes down without
any code getting worse, because it was being counted wrongly before. Writing a
`bypass` line for a guarantee that feels obvious is tedious, and the temptation
will be to write something vague to get past the gate; a vague one is worse than
none, because it looks like the work was done. Four documents where there were
two is four to keep current, and the watchlist in particular decays into a
graveyard unless entries are moved out rather than left to age.

And the measurement is now a gate too: `cargo xtask budget` records what a plan
costs, refuses a regression against what the code already did, and prints the
distance to what it ought to cost. It runs nightly rather than on the commit
path, because it needs a release build — the same trade `mutants` makes, for the
same reason.
"#],
    constraints: &[],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: slipway_core::date!(2026, 9, 5),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[],
);
