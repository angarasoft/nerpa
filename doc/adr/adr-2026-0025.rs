use crate::taxonomy::Subsystem;
use slipway_core::{nonempty_str, taxon};
use slipway_knowledge::{Breaking, DocStatus};

slipway_knowledge::adr!(
    title: "The inventory is known; facts are not",
    status: DocStatus::Active,
    subsystems: &[taxon!(Subsystem, Core)],
    context: r#"## Context

A fleet cannot be a hundred declarations. Ansible's answer is the inventory —
groups, groups of groups, values attached to either — and it is one of the best
things about it. What is not good is how values reach a task: twenty-two
precedence levels, resolved silently, so that answering "where did this value come
from" means consulting a table in the documentation.

There is also a question the inventory raises for this design specifically.
[0006](0006-static-plan-staged-execution.md) says composition is
known before anything runs, and iteration over a deferred handle is refused. But
`for node in group("web")` is iteration, and building the graph for a fleet is
exactly that.
"#,
    decision: r"
The inventory is **data, settled before the configuration is evaluated**, and it
may be iterated freely. Handles may not. The line between them is not arbitrary:
the inventory says which machines exist, which is decided by a person or by the
cloud stage, while a fact says what a machine turned out to be, which is
discovered by looking at it. The first can be known before planning. The second
cannot, and [0029](0029-a-fact-is-a-value-not-a-branch.md) covers
it.

Groups carry values. Two groups giving a node the same key **is an error naming
both groups**, not a precedence resolution. A node may override its groups
explicitly, because that is a statement rather than a rank.

Dynamic inventory — asking the cloud who exists — is a `control`-domain read like
any other, and it is **materialised**: the answer is written into the plan and
covered by its hash. A plan is therefore reproducible against a recorded
inventory rather than against whatever the cloud says next.
",
    trade_offs: &[r#"
The graph for a fleet is a loop over a group, and the plan is still fully known
before anything runs. And "where did this value come from" has one answer per
value, checked mechanically.

The cost is paid by whoever has an inventory that leans on precedence today, and
it is not small. A layout where a group deliberately sets a default that another
group overrides has to be rewritten so that the overriding is explicit — several
hours for a large inventory, and there is no mechanical translation, because the
old layout does not record which collisions were intended.

Materialising a dynamic inventory has a cost of its own: a plan made against a
fleet that has since grown will not include the new machines, and applying it
succeeds while doing nothing about them. That is the correct behaviour — a plan
is a promise about a state of the world — but it means the plan must say plainly
when its inventory came from a read and when that read happened.
"#],
    constraints: &[],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: slipway_core::date!(2026, 9, 5),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[],
);
