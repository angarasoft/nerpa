# sarma

One binary that holds cloud resources and the contents of the machines they run
on in a single graph.

`sarma` plans infrastructure and configuration together, in one pass, and
applies the result over a transport that carries an identity rather than a
static key. It is built for a world where the operator is often not a person:
the plan is a typed document before it is a page of text, and a change can be
made to require a signature from somebody other than whoever proposed it.

> **Status: stage 0, not usable.** The resource model is in place and covered by
> tests. There is no planner, no provider and no command line yet — see the
> [roadmap](ROADMAP.md).

## What it is not

**sarma is not a Terraform provider host, and is not trying to become one.**

Terraform's providers are Go binaries with full process rights: any network, the
filesystem, `exec`. They receive credentials as values, from a registry you do
not control. Accepting them as a runtime would mean accepting all of that, which
is the opposite of what this tool claims to be. Their *schemas*, on the other
hand, are JSON — a complete machine-readable description of every resource type
that somebody else spent years getting right. sarma takes the schemas and leaves
the binaries.

Terraform therefore appears on the input side only: `.tf` converts, `tfstate`
imports, and neither takes part in a run afterwards.

It is also not a Kubernetes replacement, not a CMDB, and not a monitoring
system.

## Why

Provisioning and configuration have been two tools for fifteen years, and the
seam between them is where the work goes: external data sources, provisioners
nobody defends, two state mechanisms, two languages, and a plan that stops being
true at the boundary. Both halves also carry a decade of accumulated answers to
questions that no longer need asking.

sarma is an attempt at the same job with those answers removed rather than
improved on. There is no `count`, so an index cannot shift and propose
destroying a live resource. There are no workspaces, no separate module
language, no initialisation step, and no twenty-two levels of variable
precedence. A dry run does not lie about what it could not check.

The one distinction the rest is built on is where a resource's state can be
read: through an API, or only by running code on one particular machine. Naming
that difference is what lets a VM and the packages installed on it sit in one
graph, with real edges between them.

## How it works

Configuration is a Starlark program. It is evaluated with no I/O, no clock and
no network, so the same source always produces the same graph — which is what
makes a plan worth signing. Values that do not exist yet are opaque handles that
can be passed around but not branched on, so the *set* of resources is always
known before anything happens, even when individual values are not.

Planning is therefore a single pass over the whole graph. Stages exist, but they
belong to execution: a VM is created before it is configured because the graph
says so, not because the planner had to go back and think again.

## Building

```console
$ cargo test
```

The toolchain is pinned in `rust-toolchain.toml`; `rustup` will fetch it on the
first build.

## License

Apache-2.0. Use it, ship it, build products on it, run it as a service — none of
that owes anybody anything.

That is a deliberate choice rather than an oversight. The provider ecosystem this
tool is designed around has to be written by people who are not the author, and
nobody writes a plugin while wondering whether it has become a derivative work.

Said plainly so that nobody has to guess later: the engine is Apache-2.0 and
stays that way. A coordination plane — grants, third-party approval, the audit
chain, policy for several operators at once — is planned as a separate,
separately licensed component, because that is what an organisation pays for and
a single engineer does not need. Contributions are taken under a CLA so that
this split remains possible; see [CONTRIBUTING.md](CONTRIBUTING.md).
