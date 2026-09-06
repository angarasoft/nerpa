# nerpa

One binary that holds cloud resources and the contents of the machines they run
on in a single graph.

```{admonition} Not finished, and this book says where
:class: important

Every chapter carries a marker — **built**, **partly built** or **planned** —
taken from `docs/ARCHITECTURE.md` rather than written here. A chapter marked
*planned* describes something settled and not implemented. Nothing in this book
claims more than the repository does, because nothing in it was typed twice.
```

## What this book is made of

Nothing here was written for it. The chapters are the sections of the
architecture, in the order that file already puts them; each is expanded by the
documentation of the modules that named its decision record. The guarantees come
from the invariant registry, which is checked against the code in both
directions and says, for each one, how it could be broken and whether anybody
has tried.

Pages say whether they were derived or written. This one was written.

## What is guaranteed

A sample of the registry, rendered from `docs/invariants.toml`:

```{nerpa-invariant} INV-ADDR-001
```

```{nerpa-invariant} INV-PLAN-001
```

## The shape of it

```{nerpa-chapters}
```
