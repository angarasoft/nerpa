use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::DocStatus;

dacc_knowledge::rfc!(
    title: "The configuration: a deterministic Starlark evaluation whose unresolved values never decide which resources exist",
    status: DocStatus::Active,
    target: &[taxon!(Subsystem, Config)],
    goal: r"
        A configuration is a Starlark program evaluated with no I/O, no clock and
        no network, so the same source and the same fleet always yield the same
        graph. An unresolved value — a handle, a secret, or a fact read from a
        machine — may decide what an attribute says, never whether a resource
        exists. Checkable: comparing, ordering, indexing, iterating or branching
        on such a value is refused with a file and a line.
    ",
    input_contract: r"
        main.star, read and evaluated against the fleet, into a graph of
        resources, each address naming a node.
    ",
    output_contract: r"
        A graph, and the refusals that name the file and line where an
        unresolved value was asked to decide; the same source and the same fleet
        always yield the same graph.
    ",
    invariants: nonempty_str![
        "An unresolved value never determines which resources exist.",
        "A secret can be passed to a resource and nothing else: not compared, indexed, iterated or used as a condition.",
        "Something read from a machine may decide what an attribute says and never whether a resource exists.",
        "The resource graph is acyclic, and a cycle is reported deterministically.",
        "A configuration declares the versions it needs, and a build that does not satisfy it refuses.",
        "A resource marked for merging is never overwritten without a person.",
    ],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 29),
    decided_by: &[
        crate::adr::adr_2026_0005,
        crate::adr::adr_2026_0018,
    ],
);
