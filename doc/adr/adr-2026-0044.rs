use slipway_core::nonempty_str;
use slipway_knowledge::{Breaking, DocStatus};

slipway_knowledge::adr!(
    title: "A provider builds a change and concludes from its answers",
    status: DocStatus::Active,
    subsystems: &[],
    context: r"## Context

Applying went through `Provider::apply`, which was handed a resource and reached
the machine itself. One provider held the conversation, so nothing above it could
decide the order of two machines, and nothing below it could be tested without a
machine. RFC 0001 needed the engine to hold every conversation, so that machines
could be applied side by side and what was sent could be written down before it
was sent.
",
    decision: r"
A provider builds a change and concludes from the answers, and never reaches a
machine outside `read`. `prepare` returns the operations of one change, and where
a replacement stops removing and starts making. `conclude` is handed exactly the
answers to those operations and says what the resource became. `afterwards` says
what the next run does with a change a run sent and never recorded: read it, or
send it again.

The engine seals, sends, checks what came back and records. A write is answered
with a digest of the entry read back, sealed for the machine when the content is
secret, so no answer carries a secret or a large file; the engine holds what it
sent and compares. Neither `conclude` nor scheduling sees a secret value.
",
    trade_offs: &[r"
Every provider is rewritten, and every provider says per kind whether reading
confirms a change: paid now, while one provider exists. A provider can no longer
branch inside a change on what a machine answered — work that needs an
intermediate answer is split across stages. Facts are decoded twice per change,
once for each half. The protocol goes to version 5, which is one reinstall on
every machine that already has an executor. The cost is a wider contract, four methods
where there was one, and a provider that can no longer ask a machine anything while
it works.
"],
    constraints: &[],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: slipway_core::date!(2026, 9, 17),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[],
);
