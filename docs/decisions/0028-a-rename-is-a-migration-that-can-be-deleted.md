# 0028. A rename is a migration, and it says when it can be deleted

- Status: accepted
- Date: 2026-09-05

## Context

[0002](docs/decisions/0002-addresses-carry-a-slot-and-no-index.md) makes the
address the identity of a resource. That is what makes state stable and diffs
legible, and it means renaming `os:file.old` to `os:file.new` currently reads as
"delete one thing, create another" — which for a database, a disk, or a machine
is a very expensive way to improve a name.

Terraform arrived at this too and added `moved` blocks. They work. The part that
does not work is that a `moved` block can never be removed: there is no way to
know whether some other state file somewhere is still on the old name, so the
blocks accumulate for the life of the configuration, and a mature Terraform
codebase carries a sediment of renames going back years.

## Decision

A rename is a declared migration:

    moved(from = "os:file.old", to = "os:file.new")

with an identity derived from its two endpoints. **State records the migrations it
has applied**, so a second plan against the same state knows the migration is
already spent.

That single fact is what Terraform is missing, and it changes what the tool can
say. The plan reports: *this migration is already applied in this state and can be
removed from the configuration.* For one state that is a complete answer. For a
configuration used against several — development, staging, production — it is a
per-state answer, and the person deletes the declaration once every state has
reported. That is still decisively better than never knowing.

A rename must not change anything else: after it, that resource's plan is empty,
and any attribute that genuinely differs shows up as an ordinary update rather
than hiding inside the move. Renaming across a provider or a kind is refused —
`os:file.a` cannot become `yandex:instance.a`. Changing a slot is not a rename at
all: `python:3.11` and `python:3.12` are different resources, which is what slots
are for.

## Consequences

Names can be improved. That sounds small and is not: an address that cannot be
changed becomes an address nobody dares choose carelessly, and then nobody dares
choose at all.

The costs are two. State grows a section that is append-only in practice — a
record of every migration ever applied — which is small per entry and never
shrinks, and which somebody will eventually want to prune by hand. And the
"can be removed" report is honest only about the state in front of it, so a
person running against several states has to collect several answers before
deleting anything; the tool cannot do that for them without knowing about state
files it has no business knowing about.
