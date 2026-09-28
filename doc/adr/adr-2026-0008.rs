use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "Put the gate in before the code it checks",
    status: DocStatus::Active,
    subsystems: &[taxon!(Subsystem, Gate)],
    context: r"## Context

The alternative was to add checks as they became necessary. That is how tooling
usually arrives, and it is why tooling usually has to make exceptions: a check
written after the fact meets whatever grew in its absence, and spends the rest of
its life baselining it.

Lint levels are the sharpest case. Turning a strict lint on across a thousand
lines is an afternoon; across fifty thousand it is a mechanical change nobody
reviews and a temptation to `allow` until it goes quiet.
",
    decision: r"
`cargo xtask ci` exists from the start and is the only entry point, run by CI
and by developers unchanged. Workspace lints are set at their strictest now:
`unsafe_code` forbidden, `missing_docs` denied, clippy `pedantic` and `cargo`
denied, plus targeted bans on `unwrap`, `panic`, `todo`, indexing and lossy
casts. Anything that can be checked mechanically is, and
`docs/CODING_STANDARDS.md` records which rules those are.
",
    trade_offs: &[r"
The strict configuration produced sixty-two findings on the first run and caught
two problems in the gate's own code. Every one was cheap to fix at this size and
would not have been later.

The cost is that some rules must be relaxed locally, and each relaxation carries
a written reason: tests may use `unwrap` and indexing, the contract module may
panic because that is its purpose, and `xtask` may write to the terminal because
that is its output.
"],
    constraints: &[],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 4),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[],
);
