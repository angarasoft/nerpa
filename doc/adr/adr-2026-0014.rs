use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "Ship by stage, with an executable exit criterion",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    subsystems: &[taxon!(Subsystem, Gate)],
    context: r"## Context

Sprints are a coordination mechanism for teams. This project has one maintainer
who is also running a second product, so a two-week cadence would buy ceremony
and no coordination. The failure mode to guard against is different: work that
drifts for a month on a branch, and a stage declared finished because it feels
finished.
",
    decision: r"
The roadmap stages are the plan, and each has one written exit criterion that can
be executed rather than judged. A stage ends when its criterion passes on real
hardware, not when its description feels satisfied.

Between those, work in slices that end with a green `cargo xtask ci` and a
commit. A branch that lives longer than a couple of days is a slice that was cut
too large.

Releases are tags at stage boundaries, versioned `0.x` until stage 5, where a
minor bump may break anything. `CHANGELOG.md` is written as the work happens, not
reconstructed from the log. Once anything is published, `cargo-semver-checks`
joins the gate.
",
    trade_offs: &[r"
Progress is claimed against a command that either passes or does not, which is
the only honest report a solo project can give itself. There is nothing to
schedule, attend or estimate.

The cost is that a stage with a badly written criterion becomes either a
formality or an unreachable bar, so the criteria are reviewed as carefully as the
code. And the plan hash is now a release concern: a change that alters the hash
for identical input invalidates signatures in the field, and has to be called out
in the changelog whether or not the public API moved.
"],
    constraints: &[],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 4),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[],
    introduces_norm: false,
);
