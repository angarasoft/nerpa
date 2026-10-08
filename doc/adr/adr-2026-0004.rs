use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "Write providers in-tree, admit others as WASM, never as Terraform plugins",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    subsystems: &[taxon!(Subsystem, Core)],
    context: r"## Context

Terraform's registry is the largest part of its value, and being wire-compatible
with it looks like the cheapest route to an ecosystem. But its providers are Go
binaries with full process rights — any network, the filesystem, `exec` — and
they receive credentials as values, from a registry the operator does not
control. Accepting them as a runtime would contradict everything this tool
claims about trust.

Their schemas are a different matter: `terraform providers schema -json` is a
complete machine-readable description of every resource type, and it is data.
",
    decision: r"
Three tiers. First-party providers are Rust crates in this workspace.
Third-party providers are WASM components against a WIT interface, receiving
credentials as handles and reaching only the hosts their manifest declares. No
bridge to `go-plugin` is planned. Provider schemas are harvested as data at build
time, and the binaries they came from are never shipped or executed.
",
    trade_offs: &[r#"
"A provider cannot take your key" becomes a property that can be checked rather
than a promise. Providers ship as one signed file instead of a registry of
binaries, and the tool works in an air-gapped network.

The cost is that `wasi-http` covers REST clouds but not raw TCP or
client-certificate mTLS, so anything needing those stays in-tree until that
matures. And the existing ecosystem does not come for free — only the knowledge
inside its schemas does.
"#],
    constraints: &[],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 4),
    breaking: Breaking::No,
    code_refs: &[crate::anchor::provider_trait],
    related_rfcs: &[],
    introduces_norm: true,
);
