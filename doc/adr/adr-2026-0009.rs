use crate::taxonomy::Subsystem;
use slipway_core::{nonempty_str, taxon};
use slipway_knowledge::{Breaking, DocStatus};

slipway_knowledge::adr!(
    title: "Carry an identity, and let a third party sign the plan",
    status: DocStatus::Active,
    subsystems: &[taxon!(Subsystem, Transport)],
    context: r"## Context

Every tool in this space authenticates with a long-lived key that somebody put in
CI years ago and nobody has rotated. Terraform state carries passwords in clear
text, which is why it needs whole-file encryption and still leaks through every
backup. And nothing in either tool separates the person who writes a change from
the person allowed to make it.

A single-pass plan (`0006`) makes something new possible: the plan is complete
before anything happens, so it can be hashed, and a hash can be signed.
",
    decision: r"
A run carries an actor identity — a person over OIDC or Teleport, CI over
workload identity, an agent over a grant it was issued. **Static keys are not a
mode**: there is no code path that reads one, so no deadline can produce one.

A grant limits a run to named resource kinds, targets and operations, with an
expiry, and is checked by the engine rather than by the thing being limited.

Where configured, `apply` requires a detached signature over the plan hash from
somebody other than whoever proposed it. The hash covers the configuration
revision, provider versions, the state version and a digest of the observed
state — so a signature cannot be replayed against a world that has changed.

State holds references to secrets and digests of their values, never the values.
The log is an append-only hash chain.
",
    trade_offs: &[r#"
Separation of duties becomes real rather than procedural: whoever writes the
configuration cannot apply it alone, and that holds for an agent exactly as it
holds for a person. State can go in object storage without whole-file encryption,
because it has nothing to leak.

The cost is that every deployment needs an identity provider before it can do
anything at all, which is a harder first five minutes than "export a key". And
the hash covering observed state means ordinary drift invalidates a signature —
correct, but it will feel obstructive until people see why.
"#],
    constraints: &[],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: slipway_core::date!(2026, 9, 4),
    breaking: Breaking::No,
    code_refs: &[crate::anchor::state_format],
    related_rfcs: &[],
);
