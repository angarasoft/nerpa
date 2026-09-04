# 0002. Put a slot in the address and never an index

- Status: accepted
- Date: 2026-09-04

## Context

Terraform's `count` makes a resource's identity depend on its ordinal position
in a list. Removing an entry from the middle renumbers everything after it, and
the planner proposes destroying live resources that merely moved. This is not
hypothetical: it has been observed in this author's own infrastructure, where a
count-based subnet module planned to destroy a bastion's subnet.

Portage solved the adjacent problem twenty-five years ago. `SLOT` is why
`python:3.11` and `python:3.12` are two installed packages rather than a
conflict.

## Decision

An address is `provider:kind.name[:slot][@target]`. Repetition is over stable
keys, two instances of one declaration are told apart by a slot, and no index
appears anywhere — there is no field and no constructor that could hold one.

## Consequences

The renumbering failure has nowhere to originate. Two versions of anything can
coexist legitimately: an application during a migration, two clusters during a
cutover.

The cost is that a slot has to be chosen deliberately where `count` would have
been automatic, and that this scheme is frozen the moment a state file exists
outside the author's machine.
