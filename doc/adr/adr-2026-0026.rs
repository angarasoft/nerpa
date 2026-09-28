use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "A reaction is an edge, and an unpaid one is owed",
    status: DocStatus::Active,
    subsystems: &[taxon!(Subsystem, Core)],
    context: r#"- Corrected: 2026-09-11 — "a debt has to be written down before the action is
  attempted" was built as before the reaction, and a run that stopped between the
  cause and the reaction lost the debt: found by the review of RFC 0001. It is
  written before the cause is sent, once for each cause that rouses anything, and
  a reacting resource that state does not record yet is recorded from what was
  read so that it has somewhere to carry one.
- Corrected: 2026-09-11 — a reacting resource the plan could not reach, on a
  machine that did not answer or held behind one, was owed nothing while its
  cause was sent: found by the narrow review of RFC 0001. The reaction is owed on
  its record before the cause is sent, and a cause whose reactor has no record
  waits, as the owner decided.
## Context

Writing a configuration file is half the job; the process that reads it has to be
told. Ansible's handlers do this, and three things about them go wrong often
enough to be folklore.

They run at the end of the play, so a failure between the write and the end skips
them, and the machine is left with the new file and the old process — with nothing
recording that fact. `--force-handlers` exists to paper over it and changes when
handlers run rather than whether the debt is remembered.

They are found by name in a global namespace, so a typo is silence, and two roles
using "restart nginx" collide.

And nothing tells you beforehand that a restart is coming. A play that edits a
file on two hundred machines will bounce a service on two hundred machines, and
the way to find that out is to run it.
"#,
    decision: r#"
A reaction is an ordinary edge in the graph, declared by reference:

    service("nginx", restart_on = [conf])

Three properties fall out, and they are the three failures above.

Because it is a reference rather than a name, a typo is an evaluation error with
a line number, and two modules cannot collide.

Because it is an edge, it is ordered — the restart happens after the file and
before whatever depends on the service, not at the end of everything — and two
files notifying one restart coalesce into one node with two in-edges.

Because it is in the graph, it is **in the plan**. The plan says nginx will
restart and which file causes it, before anything happens.

The fourth property is the one no predecessor has. A reaction that was triggered
and did not complete — the run failed, the machine went away — is **recorded in
state as owed**. The next plan shows it as pending with the reason. There is no
way to end up with a new file, an old process, and nobody knowing.
"#,
    trade_offs: &[r"
The common footgun stops being reachable, and the expensive part of a fleet-wide
change is visible before it is paid.

The cost is in the state format and in every provider that can react: a debt has
to be written down before the action is attempted and cleared after it succeeds,
which is two extra state writes around every reaction and a crash window between
them that resolves toward reporting a debt that was in fact paid. Reporting a
false debt costs one redundant restart; missing a real one costs an outage, so
the window resolves that way deliberately.

There is also an ergonomic cost. Ansible lets a role notify a handler defined
somewhere else entirely, and that is genuinely convenient when it works.
Requiring a reference means the reacting resource and the thing it reacts to must
be visible to one another, which occasionally forces a value to be passed where
a name used to travel on its own.
"],
    constraints: &[],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 5),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[],
);
