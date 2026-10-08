use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "A cloud provider reaches its API with a synchronous HTTP client",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    subsystems: &[taxon!(Subsystem, Yandex), taxon!(Subsystem, Docs)],
    context: r"
        A control-domain resource has no machine, so its provider reaches the
        cloud API itself, from where the engine runs. The engine is synchronous:
        0031 chose threads over async, the fan-out is bounded at sixteen, and the
        Provider and Reach traits return results rather than futures. The first
        cloud is Yandex Cloud (adr-2026-069), reached over its REST API.

        The HTTP client is a new dependency, and it is a decision rather than an
        assumption: it shapes what a provider can do and what the tree must
        audit.
    ",
    decision: r"
        The provider uses ureq — a blocking HTTP/1.1 client over rustls. One
        Agent is held per provider and reused, so a batched read keeps its
        connection, and every call carries an explicit timeout.

        The client lives inside the provider crate. No HTTP type crosses into
        nerpa-core or the Provider trait, so the trait stays synchronous and the
        client is swappable without touching the engine.
    ",
    trade_offs: &[
        "A synchronous client matches a synchronous engine, and ureq carries no tokio, hyper or tower; reqwest's blocking mode ships a tokio runtime anyway, so it buys nothing here and costs a much larger dependency tree for a tool that changes infrastructure.",
        "A blocking call is one thread per request in flight; today that is bounded by the fan-out (sixteen) and by batching (one journey per provider), which is far more than control resources need. It becomes the wrong shape only when a provider wants hundreds of parallel API calls.",
        "ureq is HTTP/1.1 and has no streaming; Yandex Cloud's REST is HTTP/1.1 JSON and its long-running operations are polled, not streamed, so neither is needed now. This is the first thing to revisit.",
        "Cancellation is not cooperative — an interrupted run waits out the timeout — and the durability model (mark before, state after, the next run reconciles) makes that safe rather than merely acceptable.",
    ],
    constraints: &[
        "The HTTP client stays inside the provider crate; no HTTP type reaches nerpa-core or the Provider trait.",
        "One Agent per provider, reused for keep-alive, with an explicit timeout on every call.",
        "A credential travels as a secret reference, never as a value in state or a plan.",
        "The choice is revisited when one of the triggers holds: a provider needing hundreds of concurrent API calls, streaming or HTTP/2, or the stage-5 agent surface.",
    ],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 10, 5),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[],
    introduces_norm: false,
);
