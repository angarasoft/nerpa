use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "A run that stops holds the machines it was acting on",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    subsystems: &[taxon!(Subsystem, Core)],
    context: r"## Context

Measured on three routes: no route stops a batch that has started, and an executor
outlived its engine by up to about a minute. So a run that is killed cannot know
what the machines it was speaking to did next, and protocol 5 gives it no way to
ask. The next run must not act on that ignorance.
",
    decision: r"
A machine whose shard holds a mark in flight from a run that ended is **held**, and
so is the control domain. The next run reads nothing from it, plans nothing on it,
and says so: a plan carries what is held and what waits beside what could not be
reached, and has no number while any of them is there. Only a person releases a
machine, with a command that settles its marks: a pending entry for a kind reading
cannot confirm, an entry for removal only for one it can.

A run is alive under `lock`, held with `flock`, so the kernel releases it however
the run ends. Every stop — a signal, the first failure under `stop`, a contract
violated, a state write that failed, a panic — sets one flag, and one supervisor
ends the run: it waits a bounded time for the answers in flight, kills the
transports as process groups, flushes the recorder and writes a report naming the
machines still in flight. Signals are counted in the handler, and the third ends
the process.
",
    trade_offs: &[r"
A run that stops leaves machines a person must release, which is work a fleet's
owner did not have before. A request that never answers holds its machine's slot
until a stop ends the wait, and a call into a provider that never returns cannot be
killed at all: both are named in the report and neither has a deadline in step 1.
Two paths end without a report — the third signal and the end under a provider
call — and the marks are what recovers them. The cost is a fleet that can need a person before the
next run, and two ways for a run to end with nothing written down.
"],
    constraints: &[],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 17),
    breaking: Breaking::No,
    introduces_norm: false,
    code_refs: &[],
    related_rfcs: &[],
);
