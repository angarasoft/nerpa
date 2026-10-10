use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::DocStatus;

dacc_knowledge::rfc!(
    title: "The baseline: the built system's invariants, the contract the first slices converge to",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    target: &[taxon!(Subsystem, Core), taxon!(Subsystem, Os)],
    goal: r"
        The seven open divergences of the external review each name the invariant
        they violate, and the slice that closes them has something to converge to.
        Checkable: a divergence names an invariant stated here, and closing one is
        an empty second run against that invariant.
    ",
    input_contract: r"
        The built system at the switch: the os provider — files, directories,
        services — and the core — the graph, the planner, state and reach. The
        seven divergences reproduced by the external review, open at the switch.
    ",
    output_contract: r"
        Seven Divergence work items in two slices — three under Os, four under
        Core — each naming the invariant it violated, closed against it.
    ",
    invariants: nonempty_str![
        "An attribute a resource was born with is reported, never turned into work.",
        "Removing a resource removes that resource and nothing that was never declared.",
        "A file mode is written as text and compared as the number it spells; a number, or text that spells no mode a file can have, is refused while planning.",
        "Applying a plan and planning again produces an empty plan.",
        "A machine that did not answer is refused per resource, not per run.",
        "State holds references to secrets and digests of their values, never the values.",
        "What waits for something that cannot be planned is said to wait for it, whatever its domain, and only a machine that did not answer is reported as one.",
    ],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 28),
    decided_by: &[crate::adr::adr_2026_050],
    introduces_norm: false,
);
