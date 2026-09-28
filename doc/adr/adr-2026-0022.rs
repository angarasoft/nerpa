use slipway_core::nonempty_str;
use slipway_knowledge::{Breaking, DocStatus};

slipway_knowledge::adr!(
    title: "Say what will not break, and check it mechanically",
    status: DocStatus::Active,
    subsystems: &[],
    context: r"## Context

Every Ansible release broke somebody's playbooks, and it is worth naming why
rather than resolving to be more careful.

Ansible's interface is its module set: thousands of them, each with its own
parameters, each maintained by different people. A playbook does not declare what
it needs — it calls a module by name and hopes. What a module does is whatever
its implementation does this release, and **there is no specification to compare
against**. So a change is discovered at runtime, on a machine, by the person
least able to do anything about it.

Three things are missing there, not one: a declared dependency, a contract that
can be compared between versions, and a way to find out before the run.
",
    decision: r"
**The plan is the compatibility surface.** For a given configuration, state and
set of providers, the plan is what this promises to keep stable. Not the
internals, not the wording of the human output — the typed plan. The same
statement the wire protocol already makes about itself in `0010`.

Three mechanisms hold it, and none of them is a resolution to be careful.

**A configuration declares what it needs.** A program says which version of nerpa
and which provider versions it expects, and a build that does not satisfy it
**refuses** rather than behaving differently. That turns a silent change of
behaviour into a stated one, before anything is touched.

**Provider schemas are compared between versions.** They are data (`0004`), so a
dropped attribute or a changed type is a diff a machine can take, in CI, rather
than a surprise in production.

**A corpus of configurations with their plans, recorded.** Every shape this
supports has its plan checked in. A version that changes any of them either has a
bug or has made a decision, and either way it has to say so — the changelog entry
is not a courtesy, it is the thing that failed the build until it was written.
`docs/TESTING.md` already treats the plan hash this way for one case; this is the
same rule at fleet scale.
",
    trade_offs: &[r"
What is promised is stated rather than implied, and the promise is checked by
something that does not get tired. A change in behaviour becomes visible at the
moment it is written instead of the moment somebody's machines misbehave.

The costs are real and worth naming. A corpus is a maintenance burden, and a
badly chosen one ossifies: recorded plans for shapes nobody uses turn every
deliberate improvement into a chore of re-recording. So the corpus grows from
configurations that exist rather than ones that might.

Refusing to run on a version mismatch will, at some point, refuse a run somebody
needed. That is the trade being made deliberately: a refusal is recoverable in a
minute, and a silent difference in behaviour is discovered at the worst possible
time.

And a promise about plans is not a promise about outcomes. A provider whose
underlying API changes can produce the same plan and a different world. What is
claimed here is the boundary this project controls, and claiming more would be
the sort of thing that gets discovered as untrue.
"],
    constraints: &[],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: slipway_core::date!(2026, 9, 5),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[],
);
