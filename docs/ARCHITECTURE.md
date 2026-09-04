# Architecture

The decisions below are settled. Each one names what it costs, because most of
them were chosen by refusing something rather than by adding it.

## Execution domains

Every resource says where its state can be read.

| Domain | Read and changed | Examples |
| --- | --- | --- |
| `control` | Through an API, from wherever the run starts | network, subnet, VM, disk, DNS record |
| `target:<node>` | Only by executing code on that node | package, file, service, user, sysctl |

This is the distinction that lets a VM and the packages on it sit in one graph.
It is also the one neither Terraform nor Ansible expresses, which is why the
seam between them exists.

## A single planning pass

Planning happens once, over the whole graph. Stages exist but belong to
execution.

This is possible because of two facts that hold together:

* The set of resources is always known after evaluating the configuration.
  Starlark returns unresolved values as opaque handles, and branching or
  iterating over a handle is rejected at evaluation time. An unknown value can
  therefore never determine which resources exist.
* For a node this run is creating, observed state is not unknown but *empty*.
  For a node that already exists, the node is reachable and can be read.

What it costs: every existing target must be readable at plan time, so
fingerprints and pushed reports are load-bearing rather than an optimisation;
plans grow with the fleet, so aggregating identical changes in the output is
mandatory; and a resource whose *existence* depends on a fact discovered on a
node that does not exist yet cannot be expressed at all.

## Addresses

```text
provider ":" kind "." name [ ":" slot ] [ "@" target ]
```

No index appears anywhere. Repetition is over stable keys, and two instances of
one declaration are told apart by a slot — the idea taken from Portage, where it
is why `python:3.11` and `python:3.12` are two packages rather than a conflict.

Addresses are frozen the moment a state file exists outside this machine, which
is why they are the first thing built.

## Zero trust

* No static keys as a mode. A run carries an identity: a person over OIDC or
  Teleport, CI over workload identity, an agent over a grant it was issued.
* A grant limits a run to named resource kinds, targets and operations, with an
  expiry. It is checked before execution, not by the thing being limited.
* A change can require a detached signature over the plan hash from somebody
  other than whoever proposed it. The hash covers the configuration revision,
  provider versions, the state version and a digest of observed state — so a
  signature cannot be replayed against a different world.
* State holds references to secrets and digests of their values, never the
  values.
* The log is a hash chain, append-only.

## Providers

Three tiers, and they coexist:

| Tier | What it is | Isolation |
| --- | --- | --- |
| in-tree | A Rust crate in this workspace | fully trusted |
| WASM | A component against a WIT interface | explicit capabilities only |
| bridge | Terraform's `go-plugin` protocol | none — not planned |

A WASM provider receives credentials as a handle rather than a value and can
reach only the hosts declared in its manifest, so "a provider cannot take your
key" is a checkable property rather than a promise. The known limit is that
`wasi-http` covers REST clouds; raw TCP and client-certificate mTLS do not fit
yet, and those stay in-tree.

## Drift

A resource may be marked so that drift stops the stage and produces a
three-way merge instead of being silently corrected. Terraform's answer to drift
is to overwrite and Ansible's is to overwrite; both are a claim about
operational reality that is untrue, because people edit machines during
incidents. Portage has had `CONFIG_PROTECT` for twenty-five years.
