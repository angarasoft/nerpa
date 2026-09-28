use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "Put a slot in the address and never an index",
    status: DocStatus::Active,
    subsystems: &[taxon!(Subsystem, Proto)],
    context: r"## Context

Terraform's `count` makes a resource's identity depend on its ordinal position
in a list. Removing an entry from the middle renumbers everything after it, and
the planner proposes destroying live resources that merely moved. This is not
hypothetical: it has been observed in this author's own infrastructure, where a
count-based subnet module planned to destroy a bastion's subnet.

Portage solved the adjacent problem twenty-five years ago. `SLOT` is why
`python:3.11` and `python:3.12` are two installed packages rather than a
conflict.
",
    decision: r"
An address is `provider:kind.name[:slot][@target]`. Repetition is over stable
keys, two instances of one declaration are told apart by a slot, and no index
appears anywhere — there is no field and no constructor that could hold one.
",
    trade_offs: &[r"
The renumbering failure has nowhere to originate. Two versions of anything can
coexist legitimately: an application during a migration, two clusters during a
cutover.

The cost is that a slot has to be chosen deliberately where `count` would have
been automatic, and that this scheme is frozen the moment a state file exists
outside the author's machine.
"],
    constraints: &[],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 4),
    breaking: Breaking::No,
    code_refs: &[crate::anchor::address],
    related_rfcs: &[],
);
