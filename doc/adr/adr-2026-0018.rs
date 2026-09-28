use crate::taxonomy::Subsystem;
use slipway_core::{nonempty_str, taxon};
use slipway_knowledge::{Breaking, DocStatus};

slipway_knowledge::adr!(
    title: "Refuse what Starlark will not, before it runs",
    status: DocStatus::Active,
    subsystems: &[taxon!(Subsystem, Config)],
    context: r#"## Context

`0006` rests on one property: an unresolved value never determines which
resources exist. The plan is computed in a single pass because the set of
resources is known after evaluation, and that is only true if a value which does
not exist yet cannot be branched on, iterated over or compared.

Building it revealed that the toolchain allows three different answers, not one.

**Refused where it happens.** Ordering comparisons, indexing and iteration all
reach the value through methods that return a `Result`, so the handle refuses
them and Starlark reports the file and line.

**Not refusable by the value.** Truthiness goes through `to_bool`, which returns
a plain `bool`: no error channel, no access to the evaluator. `if db.ip:` cannot
be stopped where it happens.

**Not consulted, but only sometimes.** Starlark defines equality between
different types as `False` rather than as an error. When the other side is a
compile-time constant the comparison is folded into a type check that never
reaches the value at all, so `db.ip == "10.0.0.1"` is silently false, the branch
silently does not happen, and a resource silently does not exist. Behind a
function parameter the literal is no longer constant, the general path runs, and
the value is consulted after all.

That distinction was not visible from the documentation and not guessable. It was
found by writing the test, watching it pass, and asking why.
"#,
    decision: r#"
Three mechanisms, one per case.

Ordering, indexing and iteration are refused by the handle, with a line.

Truthiness is recorded by the handle and the **whole evaluation is refused**
afterwards, naming the value. The graph is never returned, so the invariant
holds; only the diagnostic is weaker.

Equality and containment are refused twice over, because neither mechanism
covers the other's case. A pass over the syntax tree **before evaluation begins**
rejects any bare attribute access compared with `==`, `!=`, `in` or `not in` —
deliberately coarse, whether or not it is a resource, because in this language
`x.y` is a resource attribute while a method call is `x.y()` and an item is
`x["y"]`. And the handle keeps its own `equals`, which catches the same
comparison written where the syntax pass cannot see it.
"#,
    trade_offs: &[r"
`INV-PLAN-001` is enforced. Every shape tried is refused, and all but truthiness
name the place — the syntactic ones with a file, line and column, the runtime
ones with a traceback.

The costs. The coarse rule has false positives, and somebody comparing a bare
attribute of some future non-resource object will have to rewrite a line — chosen
over the alternative, which is a resource that quietly failed to exist. The
truthiness diagnostic names the value rather than the place. And the project now
depends on `starlark_syntax` directly, for its AST visitor.

The general lesson is worth keeping: a guarantee that rests on a dependency's
semantics has to be tested against that dependency, not assumed from its
documentation. Two of these three holes were found by writing the test and
watching it pass when it should have failed.
"],
    constraints: &[],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: slipway_core::date!(2026, 9, 5),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[],
);
