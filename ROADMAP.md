# Roadmap

Each stage is proven on a real fleet before the next one starts. The order is
not arbitrary: the parts that cannot be changed later come first.

## Stage 0 — the model and the planner

Resource graph, execution domains, single-pass planner, state that holds
references to secrets rather than their values, local transport. Starlark host
with deferred handles. An `os` provider with six primitives.

Proven on localhost, with no cloud involved.

The risk this stage retires: that the ban on branching over an unresolved value
is expressible, and that violating it produces an error naming the line.

## Stage 1 — reaching machines

The executor, `ssh` and `teleport` transports, batch introspection, fingerprints.

Proven on three to five live Gentoo nodes.

## Stage 2 — the cloud, and a way in from Terraform

Code generation from provider JSON schemas: Starlark stubs, an in-tree `yandex`
provider covering ten to fifteen types, a `tfstate` reader. Import moves secrets
out of state rather than copying them.

Proven by moving one real environment across without recreating resources.

## Stage 3 — who is allowed to do what

Grants, approval against a plan hash, the hash-chained log.

Proven by a run where one party applies and another signs.

## Stage 4 — the machine-facing surface

MCP server, typed plan, blast radius.

Proven by a first agent-driven run under a grant.

## Stage 5 — other people's providers

The WIT interface and the WASM provider loader. A separate `.tf` to `.star`
converter.

At this point the repository opens.
