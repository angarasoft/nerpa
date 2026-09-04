# 0012. Build the machine-facing surface first, not last

- Status: accepted
- Date: 2026-09-04

## Context

No infrastructure tool was designed for an operator that is not a person. Every
machine-readable output in this space is an afterthought bolted onto text meant
for humans, which is why automating Terraform means parsing prose and why
Ansible's `--check` can lie about tasks it could not evaluate.

An agent retries, loses context between runs, and cannot ask a follow-up
question. Those are the constraints that shape a good interface.

## Decision

One core library with three equal front ends: a CLI, an MCP server and an
HTTP/JSON API. **The CLI wraps the core, not the other way round**, and the MCP
server is not added later on top of parsed text.

The plan is a typed document with a versioned schema. Each change carries a blast
radius — how many nodes, whether it is reversible, whether it touches network,
access or boot — and a confidence: known, or unknown with a reference to the
resource that will supply the value. Errors are typed, with a code and a
remediation. `apply` takes an idempotency key, so a retry returns the first run's
result instead of doing the work twice. Dry run shares the code path with apply,
and a resource that cannot honestly be checked declares that in its schema rather
than reporting success.

## Consequences

An agent can decide from data instead of a regular expression, and a grant
(`0009`) gives it a boundary it cannot argue with. The human output becomes a
rendering of the typed plan rather than a separate code path that drifts from it.

The cost is that every output has two consumers from day one, and the schema is a
compatibility surface that has to be versioned like any other.
