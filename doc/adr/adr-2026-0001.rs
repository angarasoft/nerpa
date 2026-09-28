use slipway_core::nonempty_str;
use slipway_knowledge::{Breaking, DocStatus};

slipway_knowledge::adr!(
    title: "Hold cloud resources and node contents in one graph",
    status: DocStatus::Active,
    subsystems: &[],
    context: r"## Context

Provisioning and configuration have been two tools for fifteen years, and the
seam between them is where the work goes: external data sources, provisioners,
two state mechanisms, two languages, and a plan that stops being true at the
boundary. Neither Terraform nor Ansible can express the difference that causes
this, so neither can close it.
",
    decision: r"
Every resource declares an execution domain: `control`, whose state is read
through an API, or `target:<node>`, whose state can only be read by executing
code on one machine. Both kinds are nodes of one graph with real edges between
them.
",
    trade_offs: &[r"
A VM and the packages installed on it can depend on each other directly, and the
planner knows which of the two it is looking at.

The cost is that half the graph is only readable over a network to machines that
may be down, which makes reachability a planning concern rather than an
execution one. See `0006`.
"],
    constraints: &[],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: slipway_core::date!(2026, 9, 4),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[],
);
