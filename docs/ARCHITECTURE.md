# Architecture

The shape of the thing as it stands. Every section names the decision record that
settled it — this document says **what is**, the records say **why, and when**.
When they disagree, the records are right and this file is stale.

## Configuration is a Starlark program — [0005](decisions/0005-starlark-not-hcl.md)

Evaluated with no I/O, no clock and no network, so the same source always yields
the same graph. Unresolved values are opaque handles: they can be passed around
and stored in attributes, but not branched or iterated over. Modules are ordinary
functions. Terraform appears on the input side only — `.tf` converts, `tfstate`
imports, and neither takes part in a run afterwards.

The program is the source; the **resolved graph** is the artefact of record, and
that is what gets reviewed, hashed and signed.

An unresolved value is refused three different ways, because the language allows
only some of them to be precise — see
[0018](decisions/0018-what-starlark-will-not-refuse.md).

## Execution domains — [0001](decisions/0001-one-graph-two-execution-domains.md)

| Domain | Read and changed | Examples |
| --- | --- | --- |
| `control` | Through an API, from wherever the run starts | network, subnet, VM, disk, DNS record |
| `target:<node>` | Only by executing code on that node | package, file, service, user, sysctl |

The distinction that lets a VM and the packages on it sit in one graph, and the
one neither Terraform nor Ansible expresses.

## Addresses — [0002](decisions/0002-addresses-carry-a-slot-and-no-index.md)

```text
provider ":" kind "." name [ ":" slot ] [ "@" target ]
```

No index anywhere. Two instances of one declaration are told apart by a slot.
Frozen the moment a state file exists outside this machine, which is why it was
built first.

## A single planning pass — [0006](decisions/0006-static-plan-staged-execution.md)

Planning happens once, over the whole graph. Stages exist but belong to
execution. This works because the set of resources is always known after
evaluation, and because observed state for a node this run is creating is not
unknown but empty.

Unresolved values are marked and carry a reference to the resource that will
supply them. They never determine which resources exist.

A reference is unresolved in the text, not in the world: if the resource it reads
from already exists, the value is known and compared like any other. That is what
makes a second run empty, and a tool whose second run is not empty is not
convergent.

Removals come first, in reverse dependency order taken from what state recorded:
the configuration that described the dependency is exactly what was deleted. And
a resource is looked for where state left it, not where the declaration now
points — otherwise moving one reads as making a second and abandoning the first.

## A machine that will not answer — [0020](decisions/0020-a-machine-that-cannot-be-reached.md)

The unit of refusal is the resource, not the run. Everything readable is planned;
what could not be read appears saying so, carrying the machine and the age of the
newest knowledge of it, and nothing else. Unreachability travels along edges the
same way an unresolved value does.

Only "the machine did not answer" lets a run continue. Every other refusal stops
it, because treating a permission error as unreachability would quietly stop
managing a resource over a typo in an access rule.

A plan containing one of these cannot be signed: it holds an age, an age moves
every second, and a hash over it never settles. That is the intended answer —
nobody can approve a change to infrastructure they could not look at.

## Observed state — [0011](decisions/0011-observed-state-in-three-tiers.md)

Three tiers, cheapest first: a fingerprint that lets an unchanged node be skipped
whole, batch introspection that returns every host resource in one round trip,
and optional pushed reports that let planning happen without touching the node.
Load-bearing rather than an optimisation, because the plan is single-pass.

## The executor and transports — [0010](decisions/0010-static-executor-pluggable-transport.md)

A static Rust binary on the target, speaking framed CBOR over stdin and stdout.
The transport carries bytes and knows nothing about them: `teleport`, `ssh`,
`local`, `container`. The executor holds no secrets and no state beyond its
fingerprint, and is signed and verified in both directions.

## Providers — [0004](decisions/0004-providers-in-tree-and-wasm.md)

| Tier | What it is | Isolation |
| --- | --- | --- |
| in-tree | A Rust crate in this workspace | fully trusted |
| WASM | A component against a WIT interface | explicit capabilities only |
| bridge | Terraform's `go-plugin` protocol | none — not planned |

A WASM provider receives credentials as a handle rather than a value and can
reach only the hosts its manifest declares. Provider schemas are harvested from
Terraform as data at build time; the binaries they came from are never shipped.

## Zero trust — [0009](decisions/0009-carry-identity-not-keys.md)

A run carries an actor identity; static keys are not a mode. A grant limits it to
named kinds, targets and operations, with an expiry, checked by the engine. Where
configured, `apply` requires a detached signature over the plan hash from someone
other than the proposer — and that hash covers the configuration revision,
provider versions, state version and a digest of observed state, so it cannot be
replayed against a changed world. State holds references and digests, never
secret values. The log is an append-only hash chain.

## The machine-facing surface — [0012](decisions/0012-machine-facing-surface-is-first-class.md)

One core, three equal front ends: CLI, MCP server, HTTP/JSON. The plan is a typed
document carrying blast radius and confidence per change; errors are typed with a
remediation; `apply` takes an idempotency key; dry run shares the code path with
apply and declares what it could not check.

## Drift, and the rest from Portage — [0013](decisions/0013-borrow-from-portage.md)

A resource may be marked so that drift stops the stage and produces a three-way
merge rather than being silently corrected. A `pretend` phase runs during
planning and can fail it early. Variability is a declared, constrained dimension.
Executor operations run inside a declared filesystem and network scope.

Plan output follows `emerge -pv`: fixed action column, deltas rather than values,
the origin of each value, summary before detail, identical changes aggregated.

## What holds it together

`docs/invariants.toml` lists the guarantees that must not break, each naming the
code that enforces it and the tests that prove it. `cargo xtask ci` fails if
either link rots. Start there when changing anything in this document.
