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

## The inventory — [0025](decisions/0025-the-inventory-is-known-facts-are-not.md)

The inventory is data, settled before the configuration is evaluated, and it may
be iterated: `for node in group("web")` builds a graph for a fleet without
breaking the rule that composition is known. Handles and facts may not be
iterated. The line is between what a machine *is* — decided by a person or by the
cloud stage, knowable in advance — and what a machine *turned out to be*.

Groups carry values. Two groups giving one node the same key is an error naming
both, not a precedence resolution; a node may override its groups explicitly. A
dynamic inventory is a `control` read, materialised into the plan and covered by
its hash, so a plan is reproducible against a recorded fleet rather than against
whatever the cloud says next.

## Renaming — [0028](decisions/0028-a-rename-is-a-migration-that-can-be-deleted.md)

Because the address is the identity, a rename needs a declaration:
`moved(from, to)`, identified by its endpoints. State records the migrations it
has applied, which is the part Terraform lacks — so the plan can report that a
migration is spent and its declaration can be deleted, instead of accumulating
`moved` blocks forever. A rename changes nothing else: the plan for that resource
is empty afterwards, and a real difference appears as an ordinary update.

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

## Facts as values — [0029](decisions/0029-a-fact-is-a-value-not-a-branch.md)

A fact may decide what an attribute says and never whether a resource exists.
`node.fact(...)` is an unresolved value in that node's `target` domain, refused
for branching, comparison and iteration like any handle. Where a value differs by
fact, `select(fact, {closed mapping})` keeps the graph fully known and leaves only
the value open — within a set the plan can print, which says more than "known
after apply" does. A fact arriving outside the mapping fails at apply, naming the
fact, the value and the line.

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

The interface is a batch: the engine groups by machine before asking, and a
provider is handed everything on one node at once. That shape is what makes the
rest possible — an interface that can only ask about a single resource makes the
fast version unrepresentable however good the transport underneath is.

Three tiers, cheapest first: a fingerprint that lets an unchanged node be skipped
whole, batch introspection that returns every host resource in one round trip,
and optional pushed reports that let planning happen without touching the node.
Load-bearing rather than an optimisation, because the plan is single-pass.

## The executor and transports — [0010](decisions/0010-static-executor-pluggable-transport.md)

A static Rust binary on the target, speaking framed CBOR over stdin and stdout —
four bytes of length, then the body. Every frame carries the protocol version it
was written by, because an executor installed once outlives the engine that put
it there, and an unknown version is refused rather than read as a subset. The
length is checked before a byte is allocated: it is an instruction to allocate
arriving from the far end of a pipe.
The transport carries bytes and knows nothing about them: `teleport`, `ssh`,
`local`, `container`. The executor holds no secrets and no state beyond its
fingerprint, and is signed and verified in both directions.

## Configuration files — [0023](decisions/0023-templates-that-look-like-the-file.md)

Files come from Jinja templates rendered by `minijinja`, because a template that
looks like the file it produces is the reason anyone can read a configuration.
The sandbox is the same shape as Starlark's: no clock, no environment, no
filesystem, no process, no `lookup`. Values are handed over rather than inherited
from an ambient scope. Templates are read relative to the configuration's own
root, which makes the configuration a directory and makes the plan hash cover the
tree.

Rendering happens at plan time wherever the inputs are settled, so the plan shows
what a file will contain before anything happens — earlier than Ansible's
`--diff`, which reports during the run.

## Reacting to change — [0026](decisions/0026-a-reaction-is-an-edge-and-can-be-owed.md)

`service("nginx", restart_on = [conf])` is an edge, not a notification. A typo is
an evaluation error rather than silence, two files notifying one restart coalesce
into one node, the restart is ordered by the graph instead of deferred to the end
of a play, and the plan says the restart is coming before it happens.

A reaction that was triggered and did not complete is recorded in state as owed
and appears as pending in the next plan. The state Ansible reaches when a play
fails between the write and its handlers — new file, old process, no record — is
not reachable here.

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

## Secrets — [0027](decisions/0027-secrets-are-tainted-and-masked-by-span.md)

Secrecy is a taint carried by the value model: anything derived from a secret is
secret, including a rendered file. State holds a reference — provider, path,
version — and a keyed digest, never a value, because a digest answers the only
question state has, which is whether it changed. A plan reports
`password: (secret) → (secret, changed)`: enough to approve, nothing more.

A rendered file is masked by span rather than suppressed. The renderer runs twice,
once with real values and once with a random sentinel per secret, and the
difference locates where each secret landed. The diff stays readable and the
credential does not appear in it.

## The machine-facing surface — [0012](decisions/0012-machine-facing-surface-is-first-class.md)

One core, three equal front ends: CLI, MCP server, HTTP/JSON. The plan is a typed
document carrying blast radius and confidence per change; errors are typed with a
remediation; `apply` takes an idempotency key; dry run shares the code path with
apply and declares what it could not check.

## What a resource is born with — [0021](decisions/0021-attributes-a-resource-is-born-with.md)

Three kinds of attribute, not two. Converged ones are corrected in place;
immutable ones are corrected by replacement; and ones a resource was **born
with** — its image, its user data — are never corrected, because a running
machine's boot disk cannot be swapped underneath it.

Not ignored, though. The difference is reported on every plan, so a fleet running
on something no longer declared is a fact somebody is handed rather than one they
discover. Moving onto a new image is a deliberate act: replacement on demand, or
two slots side by side until the new one has shown itself.

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
