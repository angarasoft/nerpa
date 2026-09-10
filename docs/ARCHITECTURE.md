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

A name and a slot may hold a `/`; a machine may not. The two sets are not the
same one because a name reaches into a namespace somebody else owns — in portage
a package is `category/name`, three of them are called `nginx`, and a subslot is
`0/3` — while a machine is named here. An address that could not hold the
separator could not name the thing, and two declarations would collide over a
name neither of them owns
([0038](decisions/0038-two-package-families-and-a-refusal-for-the-rest.md)).

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

It is a document rather than part of the program — `--fleet` — because the
machines change without the configuration changing and one configuration runs
against several fleets. A machine joins the run's inventory when a group hands it
out, not when the document is read: two hundred described machines should not make
a run that touches three open two hundred connections.

Not built: the dynamic half. Nothing asks a cloud who exists, so nothing is
materialised yet.

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

## Facts as values — [0029](decisions/0029-a-fact-is-a-value-not-a-branch.md) · built

A fact may decide what an attribute says and never whether a resource exists.
`node.fact(...)` is an unresolved value in that node's `target` domain, refused
for branching, comparison and iteration like any handle. Where a value differs by
fact, `select(fact, {closed mapping})` keeps the graph fully known and leaves only
the value open — within a set the plan can print, which says more than "known
after apply" does. A fact arriving outside the mapping fails at apply, naming the
fact, the value and the line.

Three facts are asked: `arch`, `os_family`, and `init`. A question may take more
than one look — `init` takes two, because `/proc/1/comm` says `systemd` where
there is one and `init` everywhere else, which is what sysvinit says too. What
answers is what each leaves under `/run`. Reading it off the distribution's name
would be wrong exactly where it matters: Gentoo ships both, and the Gentoo this
was written against runs systemd.

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

A static Rust binary — 774 776 bytes, built for `x86_64-unknown-linux-musl` —
speaking framed CBOR over stdin and stdout: four bytes of length, then the body.
Every frame carries the protocol version it was written by, because an executor
installed once outlives the engine that put it there, and an unknown version is
refused rather than read as a subset. The length is checked before a byte is
allocated: it is an instruction to allocate arriving from the far end of a pipe.

The transport carries bytes and knows nothing about them, and is a command that
produces a pipe: the executor here, or `ssh` to run it there. One connection
carries a whole run.

There are two ways to run, and they are not interchangeable
([0039](decisions/0039-a-privileged-executor-is-one-the-machine-already-had.md)).
**Delivered** needs no preparation and is for unprivileged work: the executor
arrives in the connecting user's own directory. **Installed** is what escalation
requires, because an executor that user can write is one they can run as root —
so `become` without a named `executor` is refused when the fleet is read, rather
than becoming a sudoers rule that says one path and means every command there is.

The executor puts itself on a machine, by the transport's own means rather than
over the wire: until the binary is there, there is nothing to speak to. It lands
under a name carrying its digest, so a machine holding a different build holds
it under a different name and "is the right one there" is a file test rather
than a version negotiation; it is written beside and moved into place, because a
half-copied file that is executable would pass that test for ever. One
architecture, and a machine that is not it is told so by name — `W-008`.

An identity-aware proxy is not a second transport. Teleport's `tbot` writes an
OpenSSH configuration holding a `ProxyCommand` and a certificate that renews
itself, so a machine behind it is reached by the `ssh` transport being told what
file describes the way — one field in the fleet document, and no code
([0037](decisions/0037-teleport-is-a-route-not-a-transport.md)).

Built and shown against two machines reached through a bastion, and against the
same two through Teleport as a bot identity: delivery, read, plan, write, and a
second run with nothing to do.

Not built, and 0010 describes both in the present tense: the fingerprint, which
the executor refuses to invent rather than answering with a number nothing
stands behind; and **signing**. Nothing signs the executor and nothing verifies
it — the digest in its name says whether a machine already has this build, not
who made it, and a machine takes whatever the transport hands it. For a binary
that runs privileged that is a gap worth naming rather than leaving to be
inferred from a section marker.

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

A provider is the only place that can produce one, because the engine never sees
a `Seen` and a provider rebuilding the observation later would be two guesses
with nothing checking that they agree. So `Provider::read` says both what it
found and what it looked at to find it; the plan carries that, and the batch
carries it back.

What bounds this is how much a provider admits to having read. The check is
exactly as wide as that and no wider, which is a limit on the guarantee rather
than a way through it.

## Privilege — [0035](decisions/0035-privilege-belongs-to-the-transport.md) · built

Escalation is the transport's business, because the executor is a filter that
knows nothing about how it was reached:

```text
ssh web-01 sudo -n /usr/local/bin/nerpa-executor
```

No pty, no prompt, no regular expression, and nothing about privilege on the wire.
Ansible's `become` is a matrix because escalation lives inside the task layer;
here it is an argument, and arguments do not freeze. No password travels: a sudo
password is a key, and the record about keys says not to carry them.

A node says how it is reached and what it runs under, and nothing else does:

```toml
[nodes.web-01]
ssh = "web-01.example.internal"
executor = "/opt/nerpa/nerpa-executor"
become = "sudo -n"
```

Demonstrated on a machine through a bastion: a file written as root with mode
`0600`, no password anywhere, no pty and no prompt to match.

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

## Reacting to change — [0026](decisions/0026-a-reaction-is-an-edge-and-can-be-owed.md) · partly built

```python
resource("os:service.nginx@web-01", {"name": "nginx"}, reacts_to = [conf])
```

An edge, not a notification. The record's `restart_on` is spelled `reacts_to`
because what reacting means belongs to the provider — a restart for one thing is
a reload for another — and because `resource()` is the only constructor there is.

A typo is an evaluation error naming the line rather than the silence a global
handler table gives, two files rousing one service coalesce into one node with
two in-edges, the restart is ordered by the graph rather than deferred to the end
of a play, and the plan says it is coming and which file caused it:

```text
stage 2 · target: web-01
  [rouse    ] os:service.nginx@web-01
               ↺ because os:file.nginx-conf@web-01 changes
```

Not marked `!`: that mark means the change destroys what is there, and a restart
destroys nothing. What makes it visible is the line naming the cause and the
count in the summary.

A reaction that was triggered and did not complete is **owed**. The debt is
written into state before the action is attempted and gone once it succeeds, so
the window between the two resolves toward reporting a debt that was in fact
paid: one redundant restart, against an outage nobody knows about. It outlives
the process — a state file carries it — and the next plan says so with the
reason:

```text
  [rouse    ] os:file.unit@localhost
               ↺ owed since an earlier run: os:file.nginx@localhost changed and this did not follow
```

Said differently from a reaction firing now, because they are different facts
and the reader about to be woken is owed the difference.

Being roused means a restart, and only for a service that was already running:
one that was stopped before the file changed is not started by a change nobody
declared. Shown on a real machine — `chrony.conf` rewritten, `chronyd` restarted,
its process identifier different afterwards, and a second run with nothing to do
and the same identifier.

## Parallelism — [0031](decisions/0031-parallelism-is-threads-and-providers-are-sync.md) · partly built

Reading fans out over a bounded pool of sixteen: the fact journey and the
resource batches, one machine per thread, answers handed back in the order the
work was built so that a plan does not reorder itself by which machine was
quickest. Applying does not yet.

Measured on a fleet of two, it is worth nothing, and why is the finding. A bare
`ssh` behind an identity-aware proxy takes 5.07 s and behind a bastion 3.11 s;
a whole run against those machines takes 5.06 s and 3.65 s. The conversation is
half a second and the handshake is the rest — and `Piped` spawns its `ssh`
without waiting on that handshake, so every machine is already shaking hands at
once whether or not anything here is threaded. One at a time and sixteen at a
time were the same wall clock, measured both ways on one fleet. What the pool is
for is machines that work rather than wait, which is the apply half.

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

## Packages — [0038](decisions/0038-two-package-families-and-a-refusal-for-the-rest.md) · partly built

Two families and a refusal by name for the rest. `rpm` with `dnf` installs
prebuilt packages inside a transaction; `portage` with `emerge` builds from
source, keeps versions side by side in slots, and spells a name as
`category/name`. They are far enough apart that a shape surviving both will
survive `apt`, and a third family added now would prove nothing.

What a family is comes off the machine, in the fact journey that now runs before
any provider is asked. A machine calling itself something this does not know is
refused at plan time, naming the machine and what it said — no command is sent on
the chance that it works.

Presence is the declaration and there is no `state:`. `latest` is refused,
because what it does depends on when it runs and no plan can carry a number over
that. Version comparison is asked of the machine, never implemented here: `rpm`,
`portage` and `dpkg` disagree, and a comparator written on the control machine is
what `W-010` describes.

Installing and removing are built and shown on a real machine: `app-misc/jq`
installed, read back, a second run empty, then the declaration removed and the
package gone. The executor there is one root installed, per
[0039](decisions/0039-a-privileged-executor-is-one-the-machine-already-had.md).

`--oneshot` keeps a package out of portage's `world` set on purpose: the
declaration is the only record of what was asked for, and a second record that
nerpa does not write would drift without anybody seeing it. The cost is that a
package somebody installed by hand cannot be removed by `--depclean`, and the
read-back says so rather than pretending.

What a plan cannot know is whether the version it names can be installed at all —
on a machine taking only binary packages, the binhost holds what it holds. See
`W-019` and `W-020`.

## Services — [0032](decisions/0032-provider-logic-stays-on-the-control-machine.md) · partly built

Two init systems, `systemd` and OpenRC, and a refusal by name for anything else.
Which one a machine runs is the fact `init`, read before any provider is asked —
because provider logic is central, the question has to be formed before anything
is sent. Every predecessor sidesteps this by running its own code on the machine:
Ansible's module, Puppet's `confine`, Salt's `virtual()` all decide on the node.
Terraform, the one other tool whose logic is central, declines the problem and
has no service resource. What asking first buys is that the plan names the
command before it runs, which a tool deciding on the machine cannot show.

Measured on a machine of each kind, because none of it is guessable. OpenRC
answers `rc-service <name> status` with 0 running, 3 stopped and 1 for no such
service — three codes, three answers. `systemctl is-active` is the obvious
instrument and the wrong one: for a service that does not exist it prints
`inactive` and exits 3, exactly as for one merely stopped. `systemctl show`
distinguishes them with `LoadState=not-found` and answers whether it starts at
boot in the same call; OpenRC keeps that second answer as a symlink in a
runlevel, so it is an ordinary look at a path.

Whether a service is running is read back after acting, because `start` reports
success for a service that came up and fell over. Whether it starts at boot is
not: the protocol has no operation that merely looks, the set is closed on
purpose, and asking the same question two different ways in two places would be
two guesses that have to agree. Drift there is caught by the next plan's read,
like drift in anything else.

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
