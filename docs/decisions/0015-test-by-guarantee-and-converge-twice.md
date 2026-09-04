# 0015. Test by guarantee, and run every apply twice

- Status: accepted
- Date: 2026-09-04

## Context

Most of what this tool claims cannot be checked by asserting on a return value.
"The same configuration always produces the same stages", "a provider cannot see
a credential", "a second run changes nothing" are properties, not examples — and
the last of them is the definition of a convergent tool, yet no tool in this
space enforces it in its own suite.

Testing infrastructure code is also where projects quietly give up: the cloud
costs money, the machine is not there, so the provider ends up with no tests at
all.

## Decision

**The suite is indexed by guarantee.** Every entry in `docs/invariants.toml`
names the tests that prove it, and the gate fails if a named test disappears. A
test whose name does not state a guarantee does not earn its place.

**Every apply test runs twice** and asserts the second run produces an empty
plan. This is built into the harness rather than left to each test, because it is
the one property a convergent tool exists to have and the one nobody remembers to
assert.

**Determinism is a property test, not an example.** For any graph, permuting the
declaration order must not change the stages or the cycle report.

**The plan hash is a snapshot test.** A refactor that changes the hash for
identical input is a breaking change to the approval story in `0009`, and it must
show up in review as a changed file rather than be discovered in the field.

**Every provider ships a fake**, and that requirement lives in the provider trait
itself. Real providers are additionally tested against recorded HTTP exchanges,
recorded once and replayed in CI. Executor behaviour is tested in containers, one
per supported distribution.

**Mutation testing gates the suite.** A test suite that survives mutation is
decoration.

## Consequences

The properties that are hard to test are the ones that get tested, and coverage
stops being the measure. Requiring a fake at the trait level makes providers
testable by construction instead of by discipline.

The cost is real and worth naming: recorded exchanges go stale against a cloud
that changes underneath them, and refreshing them needs an account and money.
Mutation testing is minutes rather than seconds, so it runs nightly rather than on
every push. And a fake per provider is code that has to be written and kept
honest, which is a tax on every provider author, including the ones outside this
repository.
