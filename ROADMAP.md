# Roadmap

Stages, not sprints — [0014](docs/decisions/0014-ship-by-stage-not-by-sprint.md).
Each carries an exit criterion that can be **executed rather than judged**, and
the risk it retires. A stage ends when its criterion passes on real hardware, not
when its description feels satisfied.

Releases are tags at these boundaries. Versions stay `0.x` until stage 6, where a
minor bump may break anything.

> Reordered on 2026-09-05 by
> [0024](docs/decisions/0024-plan-for-what-they-do-well.md). Going through sixty
> properties of Terraform and Ansible showed nineteen answers of thirty against
> their mistakes and nine of thirty against their virtues — every stage here was
> about the engine, and none about the part somebody would drive. The missing work
> is now inside the stages that need it, and a stage for what cannot be changed
> later goes before the cloud.

## Stage 0 — the model and the planner ✔

Resource graph, execution domains, single-pass planner, state holding references
to secrets rather than their values, local transport. Starlark host with deferred
handles. An `os` provider covering files and directories.

Packages, services, users and sysctls moved to stage 1: all four need privileges
and a real system underneath, which contradicts "no cloud involved" and would
leave the exit criterion untestable anywhere but on one person's machine.

**Exit:** met. `nerpa apply` converges a local example from an empty machine, a
second run prints "nothing to do", `nerpa plan --json` round-trips through the
typed schema, and branching on a handle fails evaluation naming the line.

## Stage 1 — reaching machines, and something worth carrying

The executor, `ssh` and `teleport` transports, batch introspection, fingerprints.
Packages, services, users and sysctls.

And the three things that make the executor worth having, which used not to be
here at all:

* **Templates** ([0023](docs/decisions/0023-templates-that-look-like-the-file.md))
  — Jinja rendered in a sandbox, from values handed over rather than ambient,
  early enough that the plan shows what a file will contain.
* **Inventory groups** — a hundred machines cannot be a hundred declarations, and
  a fleet is the thing this stage exists to reach.
* **Reacting to change** — a configuration file that never restarts anything is a
  tool that does not work.

**Risk retired:** that an identity-aware proxy is a workable transport rather than
a special case; that batching beats a round trip per task by enough to matter; and
that a configuration file can be written, rendered and acted on end to end.

**Exit:** a templated configuration file converges on three to five live Gentoo
nodes through Teleport, twice, with the second run empty, and changing it restarts
the service that reads it. The plan shows the file's contents before anything
happens. A node taken offline produces a plan that says so and names the age of
its data. Wall-clock time is recorded against the equivalent Ansible playbook.

## Stage 2 — what cannot be changed later

Nothing here can be shown off on a fleet, and all of it costs a day now and a
migration once anybody's state file exists.

* **Secrets.** `Attribute::Secret`, state holding a reference and a digest rather
  than a value. The architecture already claims this; today a password in a
  configuration would land in state in clear text, which is the thing Terraform is
  criticised for here.
* **Renaming a resource.** Addresses are frozen by
  [0002](docs/decisions/0002-addresses-carry-a-slot-and-no-index.md) and there is
  no path from one name to another but destroy and recreate. Terraform learned
  this painfully and added `moved` blocks afterwards.
* **Facts as values.** [0006](docs/decisions/0006-static-plan-staged-execution.md)
  refused facts deciding which resources exist, and was right. A path that differs
  by distribution is a *value*, and there is no mechanism. The boundary needs a
  decision before an implementation, or in six months nobody will remember why one
  is allowed and the other is not.

**Risk retired:** that the frozen parts of the model can carry what was promised
of them.

**Exit:** a password in a configuration appears in neither the state file nor the
plan, and the resource that uses it still converges. A resource renamed in the
configuration keeps its identity and is not rebuilt. A value derived from a fact
works, and an attempt to decide a resource's existence from one is refused with a
line number.

## Stage 3 — the cloud, and a way in from Terraform

Code generation from provider JSON schemas: Starlark stubs, an in-tree `yandex`
provider covering ten to fifteen types, a `tfstate` reader. Import moves secrets
out of state rather than copying them. Machines are visited in parallel, because
a fleet is where sequential stops being acceptable.

**Risk retired:** that provider schemas carry enough to generate against, and that
importing beats recreating.

**Exit:** one real environment moves across without recreating a single resource;
`nerpa plan` immediately after import is empty. Import reports every secret it
found in clear text in the old state. Recorded-exchange tests cover the provider.

## Stage 4 — who is allowed to do what

Grants, approval against a plan hash, the hash-chained log.

**Risk retired:** that separation of duties survives contact with a real change,
and that a signature over observed state is not so brittle as to be unusable.

**Exit:** a run where one party applies and another signs, on the live fleet. An
apply with a signature taken before an out-of-band change is refused, naming what
moved. The log verifies from the first entry.

## Stage 5 — the machine-facing surface

MCP server, typed plan, blast radius.

**Risk retired:** that an agent can decide from the plan document alone, and that a
grant actually confines one.

**Exit:** an agent plans and applies a change end to end under a grant, with a
human signing the hash. A deliberately over-reaching request is refused by the
grant rather than by the agent's judgement.

## Stage 6 — other people's work

The WIT interface and the WASM provider loader. A separate `.tf` to `.star`
converter. Packaging and versioning for shared configuration, without which
"a module is a function" is true and useless.

**Risk retired:** that a sandboxed provider can do real work, that the capability
boundary holds, and that somebody outside this repository can share something.

**Exit:** a provider built outside this repository loads, creates a resource, and
demonstrably cannot read a credential value or reach a host outside its manifest.

At this point the repository opens.

## Not scheduled

Reading a resource without taking it under management — Terraform's data sources —
and one-shot commands in the spirit of `ansible all -a uptime`. Both are real
gaps; neither blocks anything above, and the second matters more for adoption than
its size suggests.
