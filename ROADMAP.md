# Roadmap

Stages, not sprints — [0014](docs/decisions/0014-ship-by-stage-not-by-sprint.md).
Each one carries an exit criterion that can be **executed rather than judged**, and
the risk it retires. A stage ends when its criterion passes on real hardware, not
when its description feels satisfied.

Releases are tags at these boundaries. Versions stay `0.x` until stage 5, where a
minor bump may break anything.

## Stage 0 — the model and the planner

Resource graph, execution domains, single-pass planner, state holding references
to secrets rather than their values, local transport. Starlark host with deferred
handles. An `os` provider with six primitives: package, file, directory, service,
user, sysctl.

**Risk retired:** that the ban on branching over an unresolved value is
expressible at all, and that violating it produces an error naming the line.

**Exit:** `sarma apply` converges a local example from an empty machine, a second
run produces an empty plan, and `sarma plan --json` round-trips through the typed
schema. Branching on a handle fails evaluation with a line number.
`cargo xtask ci` green, no cloud involved.

## Stage 1 — reaching machines

The executor, `ssh` and `teleport` transports, batch introspection, fingerprints.

**Risk retired:** that an identity-aware proxy is a workable transport rather than
a special case, and that batching beats a round trip per task by enough to matter.

**Exit:** the same example converges on three to five live Gentoo nodes through
Teleport, twice, with the second run empty. A node taken offline produces a plan
that says so and names the age of its data, rather than a stack trace. Wall-clock
time is recorded against the equivalent Ansible playbook.

## Stage 2 — the cloud, and a way in from Terraform

Code generation from provider JSON schemas: Starlark stubs, an in-tree `yandex`
provider covering ten to fifteen types, a `tfstate` reader. Import moves secrets
out of state rather than copying them.

**Risk retired:** that provider schemas carry enough to generate against, and that
importing beats recreating.

**Exit:** one real environment moves across without recreating a single resource;
`sarma plan` immediately after import is empty. Import reports every secret it
found in clear text in the old state. Recorded-exchange tests cover the provider.

## Stage 3 — who is allowed to do what

Grants, approval against a plan hash, the hash-chained log.

**Risk retired:** that separation of duties survives contact with a real change,
and that a signature over observed state is not so brittle as to be unusable.

**Exit:** a run where one party applies and another signs, on the live fleet. An
apply with a signature taken before an out-of-band change is refused, naming what
moved. The log verifies from the first entry.

## Stage 4 — the machine-facing surface

MCP server, typed plan, blast radius.

**Risk retired:** that an agent can decide from the plan document alone, and that
a grant actually confines one.

**Exit:** an agent plans and applies a change end to end under a grant, with a
human signing the hash. A deliberately over-reaching request is refused by the
grant rather than by the agent's judgement.

## Stage 5 — other people's providers

The WIT interface and the WASM provider loader. A separate `.tf` to `.star`
converter.

**Risk retired:** that a sandboxed provider can do real work, and that the
capability boundary holds.

**Exit:** a provider built outside this repository loads, creates a resource, and
demonstrably cannot read a credential value or reach a host outside its manifest.

At this point the repository opens.
