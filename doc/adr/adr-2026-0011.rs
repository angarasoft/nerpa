use slipway_core::nonempty_str;
use slipway_knowledge::{Breaking, DocStatus};

slipway_knowledge::adr!(
    title: "Read observed state in three tiers",
    status: DocStatus::Active,
    subsystems: &[],
    context: r"## Context

`0006` made planning a single pass, which means the observed state of every
existing target has to be read before the first action. Done naively that is
hundreds of nodes times dozens of resources of SSH round trips on every `plan`,
which would make the tool unusable on exactly the fleets it is for.

Under lazy planning this was an optimisation. It is not any more.
",
    decision: r"
Three tiers, cheapest first. A **fingerprint**: the executor keeps a digest of
what it applied, one call returns one hash, and a node whose hash matches and
whose desired state has not changed is skipped whole. **Batch introspection**:
one invocation returns the observed state of every host resource on that node, in
one round trip. **Pushed reports**, optional: a node reports on a timer, so
`plan` and drift detection need not touch it at all.

A fingerprint defers a check, it does not replace one. Drift that happened
outside the executor is invisible to it, so a full read is forced periodically
and by `--deep`.
",
    trade_offs: &[r"
Planning cost stops scaling with fleet size in the common case, and pushed
reports make planning possible against nodes that are asleep or behind a
maintenance window.

The cost is a cache with all of a cache's problems: an interval to tune, a stale
window to explain, and a `--deep` that people will forget to run. The plan output
therefore prints the age of the data it used, which is not optional.
"],
    constraints: &[],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: slipway_core::date!(2026, 9, 4),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[],
);
