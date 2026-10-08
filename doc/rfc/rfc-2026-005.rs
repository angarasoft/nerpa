use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::DocStatus;

dacc_knowledge::rfc!(
    title: "The command line: two verbs, one typed plan, rendered for a person and written for a machine",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    target: &[taxon!(Subsystem, Cli)],
    goal: r"
        A command line with two verbs: `plan` says what would happen and changes
        nothing; `apply` makes it happen and writes down what it did. No third
        verb quietly does both. The plan is a typed document, and the text a
        person reads is a rendering of that document, so `--json` writes the same
        object and the two cannot come to disagree. Checkable: the text and the
        JSON are produced from one value, not two code paths.
    ",
    input_contract: r"
        A configuration (main.star) and a fleet, plus the record of what was made
        (nerpa.state.json).
    ",
    output_contract: r"
        A plan rendered as text or written as JSON — the same object — and, for
        apply, the state written back; errors are typed with a code and a
        remediation, and a resource that cannot honestly be checked says so
        rather than reporting success.
    ",
    invariants: nonempty_str![
        "The plan a person reads and the plan a machine reads are the same object.",
        "An attribute a resource was born with is reported, never turned into work.",
    ],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 29),
    decided_by: &[
        crate::adr::adr_2026_0012,
        crate::adr::adr_2026_0019,
    ],
    introduces_norm: false,
);
