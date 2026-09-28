use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "Provider logic stays on the control machine",
    status: DocStatus::Active,
    subsystems: &[taxon!(Subsystem, Core)],
    context: r"- Corrected: 2026-09-06 — the round-trip figures were stated without saying where
  they came from, which `docs/WRITING.md` refuses. Sources added; no reasoning
  changed.
## Context

The wire protocol knows only about paths:

```rust
Ask::Look { paths: Vec<String> }
Operation::{ Directory { .. }, File { .. }, Remove { .. } }
```

Stage 1 adds packages, services, users and sysctls. None of them has a path, so
somewhere between here and a working executor there is a question nothing in
thirty-one records answers: **when a resource lives on somebody else's machine,
where does the code that manages it run?**

Three answers, and the industry has taken all of them.

**Ship the logic.** Ansible builds a zip of the Python module, base64s it, sends
it down the ssh connection and runs it on the target. Salt keeps a daemon with
the modules already there. Chef runs the client on the node.

**Ship a resolved catalog and keep providers on the node.** Puppet compiles the
catalog on the server; the agent applies it using its own local providers.

**Ship a built artifact.** Colmena and deploy-rs build the closure centrally,
copy it, and run an activation script.

Nobody computes the plan centrally *and* runs the logic centrally, and the reason
is round trips: a provider that decides on the strength of a reading must either
read across the network for every decision or run where the readings are.
",
    decision: r"
Providers run on the control machine. The executor performs a small closed set of
operations and contains no provider code.

This is not a fourth position invented here. It is what
[0006](0006-static-plan-staged-execution.md) already implies. That record forbids
deciding after reading: the whole graph is read once, planned once, and applied
once. Puppet's agent re-evaluates and Ansible decides per task — for them,
central logic would mean a round trip per decision. For a single-pass planner it
means two per machine per run, `Look` and `Perform`.

The executor is a **filter**: it reads the protocol on stdin and writes it on
stdout, and knows nothing about how it was reached.
[0010](0010-static-executor-pluggable-transport.md) says the transport is
pluggable; this says the stronger thing, that **the executor owns no socket**.

That keeps three delivery models open without choosing between them today:
pushed down the connection and cached, installed as a package, or — if it ever
comes to it — wrapped in something that listens. Start with pushed and cached,
because it works on a machine with nothing on it. Installing is then a warm
cache rather than a second mode.

A listening daemon stays a separate decision and probably a refusal. With ssh and
an identity-aware proxy the authentication is somebody else's audited problem,
and [0009](0009-carry-identity-not-keys.md) is a poor record to write beside code
that authenticates its own callers. Salt's operational burden is minion keys, and
that is not an accident.
",
    trade_offs: &[r"
Two round trips a machine a run, against Ansible's five or six ssh operations per
task per host — one with pipelining. Forty tasks on a hundred hosts is four
thousand operations there and two hundred here.

Those figures are Ansible's own documented behaviour and the reason `pipelining`
exists: see
[SSH pipelining](https://oneuptime.com/blog/post/2026-02-21-how-to-use-ansible-ssh-pipelining-for-performance/view)
and
[ansible#76941](https://github.com/ansible/ansible/issues/76941)
on the overhead of building and shipping a module per task. How the module gets
there is described in
[Ansible module architecture](https://docs.ansible.com/projects/ansible/latest/dev_guide/developing_program_flow_modules.html);
Puppet's split is in
[its architecture](https://www.puppet.com/docs/puppet/6/architecture.html) and
[catalog compilation](https://help.puppet.com/core/current/Content/PuppetCore/subsystem_catalog_compilation.htm). Nothing to install, no daemon to
keep alive, no key to rotate on two hundred machines. One binary version to reason
about rather than a provider matrix per machine. And the plan is computed where
the plan is shown, so a person approves the bytes that get sent.

It also makes [0004](0004-providers-in-tree-and-wasm.md)'s sandbox tractable: a
WASM provider that can only emit operations is far easier to confine than one
running on the target, because the capability boundary *is* the operation set.

The costs are real and three of them are structural.

**The control machine has to know every target's semantics.** `emerge` against
`apt`, version comparison, USE flags — all of it here, driven by facts rather
than by looking. Ansible puts that knowledge on the target, where the system it
describes actually is. Every provider carries a distribution matrix, and being
wrong about one is discovered remotely.

**Parsing output is a compatibility surface with no local give.** `qlist -I`
prints what the version on that machine prints, and the parser is here.

**A value that only exists after an operation costs a round trip.** The uid
`useradd` mints is not knowable at plan time, so anything reading it sits in a
later stage, and each stage boundary on a machine is another journey. Typical
depth is two to four, so it is bounded — but a deep chain eats the advantage,
and the way out would be teaching the executor to resolve references inside a
batch, which turns it into an interpreter. That is the escalation to watch for.
"],
    constraints: &[],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 6),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[],
);
