use crate::taxonomy::Subsystem;
use slipway_core::{nonempty_str, taxon};
use slipway_knowledge::{Breaking, DocStatus};

slipway_knowledge::adr!(
    title: "Licence the engine under Apache-2.0",
    status: DocStatus::Active,
    subsystems: &[taxon!(Subsystem, Core)],
    context: r"## Context

The first instinct was to stop cloud providers from selling the tool, which
points at AGPL or a source-available licence. The second was to maximise the
number of people who pick it up, which points the other way. Both are legitimate
goals and they select different licences, so the goal had to be settled first.

Two facts decided it. The provider ecosystem (`0004`) has to be written by people
who are not the author, and nobody writes a plugin while wondering whether it has
become a derivative work — a copyleft licence would work against the one part of
the plan that requires other people. And the risk today is obscurity, not
commoditisation: being taken by a cloud is a problem earned by succeeding.

Sponsorship was considered as a funding model and rejected. For developer tools
it is a tip jar, not a budget.
",
    decision: r"
The engine is Apache-2.0 and stays Apache-2.0, said plainly in the README. The
coordination plane — grants, third-party approval, the audit chain, policy for
several operators — is planned as a separate component under a different licence.
Contributions are taken under a CLA.
",
    trade_offs: &[r#"
Legal review passes without friction, distributions can package it, and a
provider author has nothing to think about. The split is an honest one rather
than a feature held hostage: it falls where "a tool one engineer runs" ends and
"how an organisation governs the tool" begins.

The cost is that nothing prevents a managed offering. The CLA is the only part
that cannot be added afterwards, which is why it is here from the first patch.
"#],
    constraints: &[],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: slipway_core::date!(2026, 9, 4),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[],
);
