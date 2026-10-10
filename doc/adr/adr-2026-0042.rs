use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "A resource found as declared is adopted, and the plan says so",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    subsystems: &[taxon!(Subsystem, Core)],
    context: r"## Context

State is what lets Nerpa remove something. A resource no longer declared is
found in state, removed in the order state recorded, and forgotten. A resource
gets into state by being applied.

A declared resource that already matched its declaration was planned as
nothing. Nothing was applied, so nothing was recorded, and when its declaration
was taken away later nothing was planned: the file stayed. Whether a file could
ever be removed depended on whether it happened to match on the first run, which
nobody reading the configuration can see. The same file differing by one byte was
an update, was recorded, and could be removed.

It is also what a run that stops leaves behind. A file written by a batch whose
state write never came is found as declared the next time. RFC 0001 cited that
as recovery, and its review found it was not true: found, it was never recorded.

Terraform and Pulumi bring a resource that exists outside state in only by an
explicit import, and without one they plan to create it. Their resources are
found by identifiers a cloud assigns. Nerpa's are found by what is declared — a
path, a package name — so a resource found as declared is the one the
declaration means, and an import would ask a person to write the declaration
twice. Ansible keeps no state and removes only what a task says to remove.
",
    decision: r"
A declared resource that exists, matches its declaration and is not in state is
planned as an adoption: shown in the plan, sent nowhere, and recorded with what
was read and every edge it comes after.
",
    trade_offs: &[r"
Owned is owned however it was found. Taking a declaration away removes the thing
whether Nerpa wrote it, changed it or found it already right, and a run that
stopped after writing and before recording loses nothing: the next plan adopts
what it left.

The cost is that removal now reaches files Nerpa did not create. Declaring
`/etc/ssh/sshd_config` exactly as the distribution shipped it and deleting the
declaration later deletes the file. That was already true of a file that differed
by one byte; it is now true of all of them, and the protection is only that the
plan says so under every adoption and counts adoptions in its summary. A way to
declare that something is managed and never removed is not built.

A first run over machines that are already configured shows every declared
resource as an adoption, so the plan somebody most wants to read quickly is the
longest one they will see.

Adoptions are written to state once for the run rather than once each, because
they send nothing and lose nothing if the run stops before the write; the next
plan adopts them again. What is recorded is what was read when planning, without
a premise and without looking again. A resource that changed in between is
recorded wrongly until the next plan reads it and proposes an ordinary update.

There is one more kind of change for a reader, an agent and the plan's number to
know about, and a plan that adopts has a number it did not have before.
"],
    constraints: &[],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 11),
    breaking: Breaking::No,
    introduces_norm: false,
    code_refs: &[],
    related_rfcs: &[],
);
