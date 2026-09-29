use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "A mark says what was in flight, and what to do about it",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    subsystems: &[taxon!(Subsystem, Core)],
    context: r"## Context

0031 wrote state after each change, so what a run could lose was the change in
flight. Reading finds a file again, and a directory, and a service's settings —
but not a package, which rpm registers before its post-install runs, and not a
resource whose identity a machine mints. A run killed in the middle of one of
those left nothing saying so.
",
    decision: r"
Before a machine's next request, its shard durably holds a **mark** of the change
about to be sent: its address, kind, locator, settled non-secret values, edges,
the time, what its provider's `afterwards` said, and the entry it replaced. A mark
is **in flight** until the write that records the answer settles it, and only a
mark in flight holds its machine (0047).

A determinate answer settles it in that write. A record clears it. A failure after
operations turns it into an **entry**: pending for a kind reading cannot confirm,
and for removal only for one it can, since something may have been made. A failure
at a premise puts back whatever the shard held before this run's mark, so an entry
an earlier run left is not lost. An entry always yields a change that sends
something in the next plan, and it is marked resent only where the reading matches
the declaration.
",
    trade_offs: &[r"
A change costs one durable write while its machine has work ready. A package
interrupted mid-install is installed again, whether or not that install finished.
An entry whose resend cannot succeed is dropped by a person, with a command for
it. The format goes to version 5, and its migration is one way except through a
downgrade.
"],
    constraints: &[],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 17),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[],
);
