use crate::taxonomy::Subsystem;
use slipway_core::{nonempty_str, taxon};
use slipway_knowledge::{Breaking, DocStatus};

slipway_knowledge::adr!(
    title: "Bind a node to the resource that creates it through an inventory",
    status: DocStatus::Active,
    subsystems: &[taxon!(Subsystem, Core)],
    context: r"## Context

A target-domain address names its node as text: `os:package.nginx@web-01`.
Nothing connects that name to the resource that brings `web-01` into existence,
so nothing makes the package wait for the machine. Stages cannot be built without
that edge, and `0001` is only true on paper until it exists.

Three ways to get it. Match by name, and hope `web-01` the node and `web-01` the
instance always mean the same thing — magic that will one day match the wrong
pair silently. Make the target a reference to a resource, which would change the
address scheme that `0002` froze. Or say it out loud.
",
    decision: r"
The graph carries an **inventory**: a mapping from a target name to what provides
it, either a resource in this graph or a declaration that the node already exists
outside Nerpa's control. Edges from every resource on a node to whatever provides
that node are derived from it.

A resource on a node the inventory does not know is **refused**, not ordered last.
If Nerpa cannot say where a machine comes from, it cannot say when to touch it.

Dependencies implied by unresolved references are derived the same way: an
attribute reading a value from another resource is an edge, without anyone
writing `after`.
",
    trade_offs: &[r"
The address scheme is untouched. Machines Nerpa does not manage are expressed in
the same construct as ones it does, so a fleet that is half adopted is not a
special case. And ordering stops depending on somebody remembering to declare it,
which is where ordering bugs come from.

The cost is a second thing to declare, and a failure mode people will meet once:
a resource on a node nobody mentioned stops the plan instead of being quietly
ordered last. That is the intended reading of fail-closed, but it will read as
pedantry the first time.
"],
    constraints: &[],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: slipway_core::date!(2026, 9, 5),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[],
);
