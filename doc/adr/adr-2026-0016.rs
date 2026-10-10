use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "Prove every check can go red",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    subsystems: &[taxon!(Subsystem, Gate)],
    context: r"## Context

A check that has never been watched fail is not a check. It is a green light of
unknown provenance, and the failure mode is silent: a rule that matches nothing,
an assertion on a value that is always equal, a scanner pointed at the wrong
directory. Nobody notices, because everything is passing — which is exactly what
a broken check looks like.

The gate is the sharpest case. `cargo xtask` guards the invariant registry, the
decision records and this repository's own rules, and until now nothing guarded
it. The registry could have stopped matching anything and the only symptom would
have been a reassuring line of output.
",
    decision: r"
Two mechanisms, aimed at different failures.

**Every check is handed a fixture that breaks it.** The checks are split into
pure functions taking their input as data — registry text, records, source text —
with the filesystem left at the edges, and each rule has a test that feeds it
something wrong and asserts the specific complaint. A missing anchor, a renamed
test, an invented identifier, a supersession pointing nowhere, consequences that
never name a cost.

**Mutation testing judges the library suite.** `cargo xtask mutants` alters the
code and asks whether anything notices. A surviving mutant is a line no test
constrains, whatever the coverage figure says.

Mutation testing is minutes rather than seconds, so it runs nightly and on
demand, never on the commit path. A gate people learn to skip is worse than no
gate.
",
    trade_offs: &[r"
The checks became simpler as a side effect: making them testable meant separating
what they decide from what they read, which is the right shape anyway.

The costs are two. Fixtures are a second description of the rules, and a rule
changed in one place and not the other produces a test that passes for the wrong
reason — so a rule change touches both by design. And mutation testing needs
exclusions to stay useful: display code and the contract module are excluded,
because a mutant that changes a message or removes a deliberate panic is
reporting something already known.
"],
    constraints: &[],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 4),
    breaking: Breaking::No,
    introduces_norm: false,
    code_refs: &[],
    related_rfcs: &[],
);
