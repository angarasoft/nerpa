# 0001. Hold cloud resources and node contents in one graph

- Status: accepted
- Date: 2026-09-04

## Context

Provisioning and configuration have been two tools for fifteen years, and the
seam between them is where the work goes: external data sources, provisioners,
two state mechanisms, two languages, and a plan that stops being true at the
boundary. Neither Terraform nor Ansible can express the difference that causes
this, so neither can close it.

## Decision

Every resource declares an execution domain: `control`, whose state is read
through an API, or `target:<node>`, whose state can only be read by executing
code on one machine. Both kinds are nodes of one graph with real edges between
them.

## Consequences

A VM and the packages installed on it can depend on each other directly, and the
planner knows which of the two it is looking at.

The cost is that half the graph is only readable over a network to machines that
may be down, which makes reachability a planning concern rather than an
execution one. See `0006`.
