use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "The run is a library the front ends wrap, not a command they parse",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    subsystems: &[taxon!(Subsystem, Core), taxon!(Subsystem, Cli)],
    context: r"## Context

0012 says one core library with three equal front ends — a CLI, an MCP server
and an HTTP/JSON API — and that the CLI wraps the core, not the other way round.
Today the opposite is true in fact: the planning and applying orchestration —
reading the fleet, registering the providers, evaluating the configuration,
reading the world, planning — lives in `nerpa-cli/src/main.rs` beside the
argument parsing and the terminal rendering. An agent front end would have to
reimplement that orchestration or scrape the CLI's text, which is exactly the
failure 0012 forbids.

What belongs to a front end is what touches its own shape: the identity comes in
as a flag for the CLI and as a field for the HTTP API; the transport that opens
a machine is the executable's own (it finds its executor beside itself); the
rendering is a terminal for one and JSON for another. What belongs to the engine
is the run itself: the providers from the fleet, the graph from the
configuration, the observed world, and the plan and the apply over it.
",
    decision: r"
A new crate, `nerpa-engine`, holds the run as a library. Its `Engine` plans and
applies: given the configuration text, the fleet, the actor, the opened
machines, the state and the secrets, it registers the providers, evaluates the
graph, reads the world, and returns the typed plan or the applied run. The
transport — opening a machine over `ssh`, finding the carried executor — stays
in the front end and is passed in as `Machines`, because it is where the
executable lives and nothing else.

The CLI becomes a wrapper: parse the arguments, resolve the identity, open the
machines, call the engine, render the result. The first machine front end is
the HTTP/JSON API, because JSON is the one format every client already speaks;
the MCP server follows over the same engine, never over the CLI's text.
",
    trade_offs: &[r"
The run lives once, so a change to planning or applying reaches every front end
at once, and an agent gets the same plan a person does rather than a paraphrase.

The cost is that the engine is a new crate to keep in step with the others, and
its boundary — the transport stays out, the providers come in — has to be
defended each time somebody wants to move a line of `main.rs` into it or out of
it.
"],
    constraints: &[
        "The engine takes `Machines` as input and opens nothing itself; the transport stays in the front end.",
        "The engine returns the typed `Plan`, never a rendered string; rendering belongs to the front end.",
        "The first machine front end is HTTP/JSON over the engine; MCP is not built on the CLI's text.",
    ],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 10, 5),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[],
    introduces_norm: false,
);
