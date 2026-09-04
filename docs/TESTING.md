# Testing

Why the suite is shaped the way it is:
[0015](decisions/0015-test-by-guarantee-and-converge-twice.md). This is how to
work in it.

## The suite is indexed by guarantee, not by module

`docs/invariants.toml` is the index. Each entry names the tests that prove it, and
`cargo xtask invariants` fails if a named test disappears or is renamed. Adding a
guarantee means adding an entry, an `// ENFORCES:` anchor, and a test the entry
names — in that order, because writing the guarantee down first is what stops it
from becoming whatever the code happens to do.

A test name is a sentence about a guarantee.
`cycle_reports_do_not_depend_on_insertion_order` says what breaks if it fails;
`test_graph_2` says nothing and will be deleted.

## Layers

| Layer | What it covers | Where it runs |
| --- | --- | --- |
| Unit and property | model, graph, planner | `cargo test`, every commit |
| Snapshot | plan text, plan JSON, **plan hash** | `cargo test`, every commit |
| Provider, recorded | one real cloud API, replayed | `cargo test`, every commit |
| Provider, live | the same, against the real account | on demand, costs money |
| Executor | one container per supported distribution | CI, every commit |
| Mutation | whether the suite means anything | nightly |

### Property tests, where an example would lie

Determinism is a property. "For any graph, permuting the declaration order does
not change the stages or the cycle report" cannot be written as three examples,
and three examples are exactly what would pass while the property is broken.
Anything hashed and signed is in this category.

### The plan hash is a snapshot, on purpose

A refactor that changes the hash for identical input is a **breaking change to
the approval story**: signatures already given in the field stop matching. Making
it a checked-in snapshot means that shows up as a changed file in review rather
than as a support ticket. Updating the snapshot is allowed; doing it without
saying so in the changelog is not.

### Every apply runs twice

The harness runs every apply test a second time and asserts the plan is empty.
This is not per-test discipline, it is the harness, because convergence is the
one property this tool exists to have and the one nobody remembers to assert.

A test that cannot pass twice is describing a resource that is not idempotent,
and that is a bug in the resource, not in the test.

### Providers ship a fake

The requirement lives in the provider trait, so a provider without one does not
compile. This makes providers testable by construction rather than by discipline,
and it applies to authors outside this repository too.

Real providers are additionally tested against recorded HTTP exchanges: recorded
once against the live account, replayed in CI. Recordings go stale — refreshing
them needs an account and money, and that is a known, accepted cost.

## Contracts are not tests

`requires!`, `ensures!` and `invariant!` state what the type system cannot, and
they stay compiled into release builds. A test proves a guarantee holds for the
cases you thought of; a contract catches the case you did not, on a machine
nobody is watching.

The suite runs twice, with contracts and without, so that no test quietly depends
on a contract's side effect.

## Running it

```console
$ cargo xtask test          both configurations
$ cargo xtask ci            everything, what CI runs
$ cargo mutants             nightly in CI, occasionally worth running by hand
```

## What is not tested, and why

Live cloud runs are not on the commit path. They cost money and they fail for
reasons that have nothing to do with the change under review, which is how teams
learn to ignore a red build. They run on demand, before a stage is declared
finished — see the exit criteria in [ROADMAP.md](../ROADMAP.md).
