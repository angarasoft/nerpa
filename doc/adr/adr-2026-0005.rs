use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "Configure in Starlark, not HCL",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    subsystems: &[taxon!(Subsystem, Config)],
    context: r"## Context

HCL is what the audience already reads, and `hcl-rs` would supply a parser for
free. But HCL is a syntax, not a language: Terraform's real language is HCL plus
an evaluation model, and adopting the syntax saves a parser rather than a design.
Its own additions — `count`, `for_each`, `dynamic`, `${}` — are four mechanisms
that exist because the syntax could not grow into a language.

The deciding constraint came from elsewhere. A plan is hashed and the hash is
signed (`0007`), so the same source must always produce the same graph. HCL's
hermeticity leaks through `file()`, `timestamp()` and `uuid()` on the first day.
",
    decision: r"
Configuration is a Starlark program. Terraform appears on the input side only:
`.tf` converts and `tfstate` imports, and neither takes part in a run afterwards.
",
    trade_offs: &[r"
Starlark has no I/O, no clock, no network, and freezes values after module load,
so evaluation is reproducible — which is what makes a plan worth signing. It is
deliberately not Turing-complete, so evaluation terminates. Modules become
ordinary functions, which removes a module language, a registry and a fetch step.
And large language models write Python far more reliably than HCL, which matters
for a tool whose operator is often not a person.

The cost is real: reading a file no longer tells you which resources exist. The
program has to be evaluated, so the artefact of record becomes the resolved
graph, and that is what gets reviewed, hashed and signed — not the source.
"],
    constraints: &[],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 4),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[],
);
