# Testing

Why the suite is shaped the way it is:
`0015` and
`0016`. This is how to work in it.

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
| Gate | that every check can go red | `cargo test -p xtask`, every commit |
| Mutation | whether the suite means anything | nightly, `cargo xtask mutants` |

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

Convergence is the one property this tool exists to have, so a test that applies
anything plans again afterwards and asserts the plan is empty.

This is per-test discipline, and saying otherwise here is what it cost: this
document claimed a harness did it, no harness ever did, and packages and
services went without a second run for as long as nobody checked. A test that
applies and does not plan again is incomplete, and review is the only thing that
catches it.

A test that cannot pass twice is describing a resource that is not idempotent,
and that is a bug in the resource, not in the test.

### Adversarial tests, and why they are a separate word

A test that exercises a guarantee and a test that attacks one are different
things, and the registry now tells them apart. Every built entry in
`docs/invariants.toml` writes down its `bypass` — what an attempt to violate it
would actually look like — and an entry may only be `enforced` if its
`adversarial` list names tests that carry that attempt out and assert it is
refused.

An adversarial test is named after the route it takes, not after the property:
`a_value_that_does_not_exist_yet_cannot_be_laundered_through_a_dictionary` says
what somebody tried. A reader who wonders whether a guarantee holds can then find
out what has actually been tried against it, rather than reading a list of names
that all sound reassuring.

Where the types make a violation impossible to express, the entry says
`bypass = "unrepresentable"` and no such test is asked for. That is a real
category and not an escape hatch — there is no index in an address because no
field and no constructor can hold one, and no test can attempt what cannot be
written.

`cargo xtask invariants` refuses `enforced` without the list. What it cannot
check is whether the named tests attack the bypass the entry describes; that is
prose against prose, and it is why promotion wants a reader who did not write the
entry. Two entries reached `enforced` and came back the same day when one did.

### Providers ship a fake

The requirement lives in the provider trait, so a provider without one does not
compile. This makes providers testable by construction rather than by discipline,
and it applies to authors outside this repository too.

Real providers are additionally tested against recorded HTTP exchanges: recorded
once against the live account, replayed in CI. Recordings go stale — refreshing
them needs an account and money, and that is a known, accepted cost.

## The gate is tested too

A check nobody has watched fail is a green light of unknown provenance. So the
checks take their input as data — registry text, decision records, source text —
with the filesystem at the edges, and **every rule has a test that feeds it
something wrong and asserts the specific complaint**: a missing anchor, a renamed
test, an invented identifier, a supersession pointing nowhere, consequences that
never name a cost.

Adding a rule to `xtask` means adding the fixture that breaks it, in the same
commit. A rule without one is not enforced, it is decorative.

## Mutation testing

```console
$ cargo xtask mutants
```

It alters the code and asks whether anything notices. A surviving mutant is a
line no test constrains, whatever coverage says about it — and coverage is not
tracked here for exactly that reason.

Minutes rather than seconds, so it is nightly in CI and on demand locally, never
on the commit path: a gate people learn to skip is worse than no gate. Run it by
hand after reorganising a test suite, which is when tests most often stop
constraining what they used to.

The line drawn in `.cargo/mutants.toml` is **decision logic is mutated, plumbing
is not**. A function whose only job is to read the filesystem and hand data to a
decision has nothing a fixture could constrain, and mutating it produces
survivors that can only be silenced by testing the filesystem. The decisions
those functions feed stay in scope, and that is where the value is. Display code
is out because a changed message is not a changed behaviour, and `contract.rs` is
out because a mutant removing a deliberate panic only reports that contracts were
not load-bearing in that test — which the without-contracts suite already
establishes.

The first run is worth recording, because it is the argument for doing this at
all: **62 survivors of 153**. Nine were in formatting code the configuration was
supposed to exclude and did not, because the exclusion pattern was written the
obvious way rather than the way cargo-mutants describes a mutant. The rest were
real: `Graph::len`, `is_empty` and `get` had no test at all; `is_control` was only
ever asserted false; the charset rule could be narrowed to reject digits and
underscores without failing anything; and the cycle report could be widened to
name resources it does not hold up, because every cycle fixture had all its
resources stuck.

After those tests were written, `crates/` has no survivors.

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
