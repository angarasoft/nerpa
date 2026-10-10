use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "A fact is a value, not a branch",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    subsystems: &[taxon!(Subsystem, Core)],
    context: r#"## Context

[0006](0006-static-plan-staged-execution.md) refuses to let a fact
gathered from a machine decide which resources exist, and that refusal is what
makes a single-pass plan possible. It is right and it stays.

But it leaves a gap that every real configuration hits within a day. The path to
a web server's configuration differs between distributions. The name of a package
differs. The address of the default interface is discovered, not declared. None
of these decide *whether* a resource exists — the resource exists either way. They
decide what one of its attributes says.

Conflating the two is why the refusal currently reads as "facts are not
supported", which is not the intent and would make the tool unusable on a mixed
fleet.

## Decision boundary

The rule is one sentence: **a fact may decide what an attribute says and never
whether a resource exists.** Written down now because in six months the line
between the allowed `select` and the refused `if` will look arbitrary to somebody
holding only the error message.
"#,
    decision: r#"
`node.fact("os_family")` yields an unresolved value in that node's `target`
domain. It may be passed as an attribute value. It may not be branched on,
compared or iterated — the same refusals handles already carry, for the same
reason.

Where a value genuinely differs by fact, the mechanism is a closed mapping:

    path = select(node.fact("os_family"), {
        "gentoo": "/etc/nginx/nginx.conf",
        "debian": "/etc/nginx/sites-enabled/default",
    })

Every branch is present in the configuration, so the graph is fully known: one
resource, one edge, one place in the order. Only the value is unsettled, and it is
unsettled within a set the plan can print — `path: one of {…}` — which is a more
useful thing to tell somebody than "unknown". A fact arriving outside the mapping
is an error at apply naming the fact, its value and the line, rather than a silent
default.

Gathering needs no new machinery: facts are readings, and the observe phase
already collects readings per machine in one round trip.
"#,
    trade_offs: &[r"
Mixed fleets work, and the plan stays fully known — the bounded unknown is
strictly more informative than Terraform's `(known after apply)`, which names no
possibilities at all.

The costs. A closed mapping is more typing than an `if`, and it forces every case
to be written out even where a person knows only one will ever be taken — which is
the intent, and will still feel like bureaucracy on a fleet that is in fact
uniform. Bounded values propagate: a path built from a `select` is bounded, a file
at that path is bounded, and a plan against a large mixed fleet can end up
printing a great many alternatives, so the renderer has to collapse them by group
rather than listing every combination. And a fact whose value set is genuinely
open — a kernel version, an interface address — does not fit a mapping at all;
those may be used as values directly and simply print as unresolved, so the
mechanism helps exactly where the possibilities are enumerable and nowhere else.
"],
    constraints: &[],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 5),
    breaking: Breaking::No,
    introduces_norm: false,
    code_refs: &[],
    related_rfcs: &[],
);
