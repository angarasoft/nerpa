# Architecture

The shape of the thing as it stands. Every section names the decision record that
settled it — this document says **what is**, the records say **why, and when**.
When they disagree, the records are right and this file is stale.

Every heading also carries one of three words, and they are the most important
thing on the page:

* **built** — the code is there, and the sections below describe it.
* **partly built** — some of what follows exists and some does not. The section
  says which.
* **planned** — settled by its record, written here in the present tense because
  that is how a contract reads, and **not implemented**. Nothing below such a
  heading is a description of this program.

The markers exist because a document written entirely in the present tense cannot
be told apart from a working system, and this one describes rather more than has
been built. `cargo xtask docs` fails if a heading loses its marker.

## Configuration is a Starlark program — [0005](decisions/0005-starlark-not-hcl.md) · built

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

## Execution domains — [0001](decisions/0001-one-graph-two-execution-domains.md) · built

| Domain | Read and changed | Examples |
| --- | --- | --- |
| `control` | Through an API, from wherever the run starts | network, subnet, VM, disk, DNS record |
| `target:<node>` | Only by executing code on that node | package, file, service, user, sysctl |

The distinction that lets a VM and the packages on it sit in one graph, and the
one neither Terraform nor Ansible expresses.

## Addresses — [0002](decisions/0002-addresses-carry-a-slot-and-no-index.md) · built

```text
provider ":" kind "." name [ ":" slot ] [ "@" target ]
```

No index anywhere. Two instances of one declaration are told apart by a slot.
Frozen the moment a state file exists outside this machine, which is why it was
built first.

## The inventory — [0025](decisions/0025-the-inventory-is-known-facts-are-not.md) · partly built

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

## Renaming — [0028](decisions/0028-a-rename-is-a-migration-that-can-be-deleted.md) · planned

Because the address is the identity, a rename needs a declaration:
`moved(from, to)`, identified by its endpoints. State records the migrations it
has applied, which is the part Terraform lacks — so the plan can report that a
migration is spent and its declaration can be deleted, instead of accumulating
`moved` blocks forever. A rename changes nothing else: the plan for that resource
is empty afterwards, and a real difference appears as an ordinary update.

## A single planning pass — [0006](decisions/0006-static-plan-staged-execution.md) · built

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

## Facts as values — [0029](decisions/0029-a-fact-is-a-value-not-a-branch.md) · planned

A fact may decide what an attribute says and never whether a resource exists.
`node.fact(...)` is an unresolved value in that node's `target` domain, refused
for branching, comparison and iteration like any handle. Where a value differs by
fact, `select(fact, {closed mapping})` keeps the graph fully known and leaves only
the value open — within a set the plan can print, which says more than "known
after apply" does. A fact arriving outside the mapping fails at apply, naming the
fact, the value and the line.

## A machine that will not answer — [0020](decisions/0020-a-machine-that-cannot-be-reached.md) · built

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

## Observed state — [0011](decisions/0011-observed-state-in-three-tiers.md) · partly built

The interface is a batch: the engine groups by machine before asking, and a
provider is handed everything on one node at once. That shape is what makes the
rest possible — an interface that can only ask about a single resource makes the
fast version unrepresentable however good the transport underneath is.

Three tiers, cheapest first: a fingerprint that lets an unchanged node be skipped
whole, batch introspection that returns every host resource in one round trip,
and optional pushed reports that let planning happen without touching the node.
Load-bearing rather than an optimisation, because the plan is single-pass.

## The executor and transports — [0010](decisions/0010-static-executor-pluggable-transport.md) · partly built

A static Rust binary — 767 KB — speaking framed CBOR over stdin and stdout —
four bytes of length, then the body. Every frame carries the protocol version it
was written by, because an executor installed once outlives the engine that put
it there, and an unknown version is refused rather than read as a subset. The
length is checked before a byte is allocated: it is an instruction to allocate
arriving from the far end of a pipe.
The transport carries bytes and knows nothing about them: `teleport`, `ssh`,
`local`, `container`. The executor holds no secrets and no state beyond its
fingerprint, and is signed and verified in both directions.

## Where provider logic runs — [0032](decisions/0032-provider-logic-stays-on-the-control-machine.md) · built

Providers run where the engine runs. The executor performs a small closed set of
operations and contains no provider code, so a provider is a translator: readings
become things to look at, a change becomes things to do, and the answers become
attributes.

Nobody else plans centrally and runs the logic centrally. Ansible ships the module
and runs it there; Salt keeps a daemon that already has it; Puppet compiles a
catalog centrally and applies it with providers on the node. The reason they
cannot is round trips — a provider deciding on the strength of a reading needs one
per decision — and a single planning pass already forbade deciding after reading.

Two journeys a machine a run rather than one per task. Nothing to install, no
daemon to keep alive, no key to rotate on two hundred machines. The cost is that
this machine has to know `emerge` from `apt`, that parsing output has no local
give, and that a value which only exists after an operation costs a round trip.

## What the wire may be asked — [0033](decisions/0033-process-is-a-providers-word-not-an-authors.md) · partly built

The operation set is small enough to freeze because it is general, and general
means it includes running a program: installing a package *is* running `emerge`.
Reading is the same shape, because a provider needs `qlist -I` to know what is
installed as much as `emerge` to change it.

`Operation::Process` is emitted by providers and never written by the author of a
configuration. Write it into the language and this is Terraform's `remote-exec`,
with every criticism of it deserved: not idempotent, never recorded in state, a
plan that cannot say what a script will change. The distinction is not whether a
process runs — Ansible's `package` module runs `apt-get` — but who wrote the
command and whether the plan describes its effect.

The wire carries it and the executor runs it; no provider emits one yet.

## A plan carries its premise — [0034](decisions/0034-a-plan-carries-its-premise.md) · partly built

Between looking and acting the machine can change, and reading everything once
makes that window the whole run where Ansible's is one task. So a batch carries
the premise — a digest of every observed value the plan was computed from — and
the far end looks again before it does anything, refusing the whole batch if any
of it moved. Not failing: `Moved` says the plan is stale, which is different
advice.

It is also what makes a signature mean something. Signing a plan whose premise
cannot be checked is signing *do this, whatever is there*.

The protocol carries it and the executor checks it; the planner does not yet
produce one.

## Privilege — [0035](decisions/0035-privilege-belongs-to-the-transport.md) · planned

Escalation is the transport's business, because the executor is a filter that
knows nothing about how it was reached:

```text
ssh web-01 sudo -n /usr/local/bin/nerpa-executor
```

No pty, no prompt, no regular expression, and nothing about privilege on the wire.
Ansible's `become` is a matrix because escalation lives inside the task layer;
here it is an argument, and arguments do not freeze. No password travels: a sudo
password is a key, and the record about keys says not to carry them.

Nothing escalates yet, because nothing has reached a second machine.

## Configuration files — [0023](decisions/0023-templates-that-look-like-the-file.md) · partly built

Files come from Jinja templates rendered by `minijinja`, because a template that
looks like the file it produces is the reason anyone can read a configuration.
The sandbox is the same shape as Starlark's: no clock, no environment, no
filesystem, no process, no `lookup`. Values are handed over rather than inherited
from an ambient scope. Templates are read from the directory the configuration
sits in and nowhere else, which makes a configuration a tree rather than a file.

Every filter a template may use is a list this project keeps rather than whatever
the library ships, because each one is part of what a configuration may rely on
and an upgrade should not widen that silently. Execution is bounded: fuel and a
recursion limit, since a template arriving from somebody's repository is exactly
where a loop that does not end should be impossible.

Rendering happens during evaluation, before planning, so the plan shows what a
file will contain — as lines rather than as one escaped string — before anything
happens. Ansible's `--diff` reports during the run.

Not built: a template whose values are not all settled. `0023` says such a
template renders during apply from what the earlier stage produced; today a value
a template cannot be shown is refused where it is written, which is loud rather
than silent but is not the whole of what was decided.

## Reacting to change — [0026](decisions/0026-a-reaction-is-an-edge-and-can-be-owed.md) · planned

`service("nginx", restart_on = [conf])` is an edge, not a notification. A typo is
an evaluation error rather than silence, two files notifying one restart coalesce
into one node, the restart is ordered by the graph instead of deferred to the end
of a play, and the plan says the restart is coming before it happens.

A reaction that was triggered and did not complete is recorded in state as owed
and appears as pending in the next plan. The state Ansible reaches when a play
fails between the write and its handlers — new file, old process, no record — is
not reachable here.

## Parallelism — [0031](decisions/0031-parallelism-is-threads-and-providers-are-sync.md) · partly built

Threads, not async. The workload is hundreds of conversations that spend their
time waiting rather than tens of thousands, and `async` would colour the one
signature strangers write against — a provider author would have to bring a
runtime, and a WASM provider would have to bridge one across the sandbox boundary
that exists to keep things out.

So `Provider` is `Send + Sync`, added while there are no providers outside this
repository and the change costs nothing. Fan-out is bounded, planning stays
single-threaded and stays first, results are collected in a fixed order, and state
is written from one place.

The bound is there and nothing is parallel yet, which is the risk the invariant
names: it is load-bearing only at stage 3, so until then it looks like something
nobody needs.

## Providers — [0004](decisions/0004-providers-in-tree-and-wasm.md) · partly built

| Tier | What it is | Isolation |
| --- | --- | --- |
| in-tree | A Rust crate in this workspace | fully trusted |
| WASM | A component against a WIT interface | explicit capabilities only |
| bridge | Terraform's `go-plugin` protocol | none — not planned |

A WASM provider receives credentials as a handle rather than a value and can
reach only the hosts its manifest declares. Provider schemas are harvested from
Terraform as data at build time; the binaries they came from are never shipped.

## Zero trust — [0009](decisions/0009-carry-identity-not-keys.md) · planned

A run carries an actor identity; static keys are not a mode. A grant limits it to
named kinds, targets and operations, with an expiry, checked by the engine. Where
configured, `apply` requires a detached signature over the plan hash from someone
other than the proposer — and that hash covers the configuration revision,
provider versions, state version and a digest of observed state, so it cannot be
replayed against a changed world. State holds references and digests, never
secret values. The log is an append-only hash chain.

## Secrets — [0027](decisions/0027-secrets-are-tainted-and-masked-by-span.md) · partly built

`secret("lockbox", "prod/db/password")` names a value without reading it. What
comes back can be given to a resource and nothing else — comparison, indexing,
iteration and truthiness are all refused, by the same three mechanisms
[0018](decisions/0018-what-starlark-will-not-refuse.md) needed for handles and for
the same reasons.

An attribute holding one is `Attribute::Secret`, and it answers `None` to
`known()` — the accessor every part of the engine that persists or prints goes
through. State keeps `Recorded`, which is either a value or a keyed digest and
has no path back to a value, so "state never holds a credential" is a property of
the types rather than a rule to remember. In the state file the digests sit in a
map of their own, so `jq .resources[].attributes` shows a person that none of it
is a credential.

Two places see a real value: the planner, which fetches it to hash it and drops
it, and the step that hands a provider what to write. A plan reports
`password: (secret) → (secret, changed)` — enough to approve, nothing more — and
a store that cannot answer stops the plan rather than reporting "unchanged",
which is the answer that would be believed and could be wrong.

Two parts of `0027` arrive with templating, because they have nothing to act on
until a template exists: taint propagating through derivation, and masking a
rendered file by span rather than suppressing it.

## The machine-facing surface — [0012](decisions/0012-machine-facing-surface-is-first-class.md) · partly built

One core, three equal front ends: CLI, MCP server, HTTP/JSON. The plan is a typed
document carrying blast radius and confidence per change; errors are typed with a
remediation; `apply` takes an idempotency key; dry run shares the code path with
apply and declares what it could not check.

## What a resource is born with — [0021](decisions/0021-attributes-a-resource-is-born-with.md) · built

Three kinds of attribute, not two. Converged ones are corrected in place;
immutable ones are corrected by replacement; and ones a resource was **born
with** — its image, its user data — are never corrected, because a running
machine's boot disk cannot be swapped underneath it.

Not ignored, though. The difference is reported on every plan, so a fleet running
on something no longer declared is a fact somebody is handed rather than one they
discover. Moving onto a new image is a deliberate act: replacement on demand, or
two slots side by side until the new one has shown itself.

## Drift, and the rest from Portage — [0013](decisions/0013-borrow-from-portage.md) · partly built

A resource may be marked so that drift stops the stage and produces a three-way
merge rather than being silently corrected. A `pretend` phase runs during
planning and can fail it early. Variability is a declared, constrained dimension.
Executor operations run inside a declared filesystem and network scope.

Plan output follows `emerge -pv`: fixed action column, deltas rather than values,
the origin of each value, summary before detail, identical changes aggregated.

## What holds it together · built

`docs/invariants.toml` lists the guarantees that must not break, each naming the
code that enforces it and the tests that prove it. `cargo xtask ci` fails if
either link rots. Start there when changing anything in this document.
