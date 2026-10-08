use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "`process` is a provider's word, never an author's",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    subsystems: &[taxon!(Subsystem, Os)],
    context: r"## Context

[0032](0032-provider-logic-stays-on-the-control-machine.md) puts provider logic
on the control machine and leaves the executor performing operations. That only
works if the operation set is general enough for every provider anybody will ever
write — including providers written outside this repository, which
[0004](0004-providers-in-tree-and-wasm.md) says there will be.

Growing the set per resource kind does not work. It makes the wire the union of
every provider that has ever existed, and rule 5 freezes it: a third-party
provider could never add the operation it needs.

So the set has to be small and general, and general means it includes running a
program. Installing a package *is* running `emerge`. There is no honest way to
describe it that does not end there.

That lands us next to something the industry has already judged. Terraform's
`remote-exec` provisioner is the standard example of what not to do: not
idempotent, never recorded in state, `terraform plan` cannot say what the script
will change, and a failure leaves the resource tainted.
",
    decision: r#"
The operation set is small and closed, and it includes process execution:
paths — write, remove, permissions — and a process with argv, stdin and expected
exit codes.

**`Operation::Process` is emitted by providers and never written by the author of
a configuration.** No `process(...)` in Starlark, not now and not as a
convenience later.

The distinction the `remote-exec` critique turns on is not whether a process
runs. Ansible's `package` module runs `apt-get`, and nobody calls that an
anti-pattern. It is **who wrote the command and whether the plan describes its
effect**. A person writes `package: nginx`; the module owns the shelling out; the
plan is about nginx. The moment a person writes the command instead, every one of
those criticisms lands, and it lands on us.

Two properties follow and both are load-bearing:

* A provider that emits a process is answerable for it being safe to re-run, and
  for the plan line above it describing what it does.
* A configuration that cannot say "run this" cannot be used to run this. That is
  the point. An escape hatch is only an escape hatch until somebody's playbook
  depends on it.

Anything secret goes to a process on **stdin, never in argv** — `/proc/*/cmdline`
is world-readable — and is marked so it never reaches the log that
[0009](0009-carry-identity-not-keys.md) chains.
"#,
    trade_offs: &[r#"
Any provider can be written without changing the wire, which is what lets the
wire freeze. The sandbox for a third-party provider has a name: the operation
set is the capability list, and a provider that can only emit operations cannot
do anything that is not on it.

The costs.

**Whoever can send `Perform` can run anything.** This is a remote shell with
better manners, and pretending otherwise would be theatre — Ansible, Salt, Chef
and Puppet all have this property, because installing software is executing code.
What limits it is not the protocol but the identity that reached it, the grants
in `0009`, and the plan the batch came from. Those have to be real, and until
they are, this is the largest authority in the system.

**Somebody will ask for the escape hatch**, with a good reason, on a deadline.
Refusing then will look like pedantry, which is why the refusal is written down
now, before there is a person on the other side of it.

**Providers carry the burden that used to be the module's.** Idempotence, exit
code interpretation, and knowing that `emerge` returns 1 for both "already
installed" and "the world is broken" — all of that is provider work, in the
provider's tests, with no local re-evaluation to fall back on.

**Partial failure is coarse.** A batch stops at the first failure and reports how
many operations completed; what a half-finished `emerge` left behind is not
something the executor can describe. Ansible has the same problem and the same
answer: the next `Look` tells the truth. The cost is that a run that stopped is
not a run whose state can be inferred — it has to be re-read.
"#],
    constraints: &[],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 6),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[],
    introduces_norm: false,
);
