use slipway_core::nonempty_str;
use slipway_knowledge::{Breaking, DocStatus};

slipway_knowledge::adr!(
    title: "Parallelism is threads, and providers are `Sync`",
    status: DocStatus::Active,
    subsystems: &[],
    context: r#"- Corrected: 2026-09-17 — "state is written from one place" was built as one
  writer of one file, which made every machine wait for every other machine's
  write. 0045 keeps the recorder as the one place that decides the order and
  gives each machine's shard a writer of its own.
## Context

Stage 3 says machines are visited in parallel. Today `Providers` holds
`Box<dyn Provider>` with neither `Send` nor `Sync` on it, and the one provider
that exists keeps a `RefCell`. Those two facts make the promise unimplementable
without changing the trait — and the trait is the one
[0004](0004-providers-in-tree-and-wasm.md) says people outside this repository
will write against, opened to them at stage 6.

Right now there are no providers outside this repository, so the change costs
nothing. After stage 6 it is a breaking change to somebody else's code, and the
people it breaks will not be in the room to argue.

The choice is threads with `Sync` providers, or async.
"#,
    decision: r"
Threads. `Provider: Send + Sync`, and the bound goes on now.

Execution fans out over machines with a bounded pool. One machine's batch is one
blocking conversation, which is what ssh and an identity-aware proxy already are.

Three reasons, in the order they carry weight.

**The workload is hundreds of conversations that spend their time waiting, not
tens of thousands.** Threads are adequate at that scale, and adequacy is the
entire argument for the simpler of two things.

**`async` colours every signature it reaches**, and the signature it would reach
here is the one strangers write against. A provider author would have to bring a
runtime; a WASM provider would have to bridge one across a sandbox boundary that
exists to keep things out.

**Blocking is what the transports are.** Wrapping a blocking ssh conversation in
an async interface buys the appearance of concurrency and none of it.

What follows for the shape of execution, so that it is decided here rather than
discovered later:

* Parallelism is **bounded**. Two hundred machines must not mean two hundred
  simultaneous handshakes from the control machine.
* **Planning stays single-threaded and stays first.** Parallelism is in execution
  only, which [0006](0006-static-plan-staged-execution.md) already implies by
  putting stages there.
* **Results are collected in a fixed order**, so the same plan applied twice
  reports the same way round. A run whose output depends on scheduling is a run
  nobody can diff.
* **State is written from one place.** `INV-STATE-005` records after every
  change; with several machines changing at once that becomes a serialisation
  point — which is a second reason it is a callback the caller owns rather than a
  path the engine writes to.

`Os` swaps its in-memory `RefCell` for a `Mutex` in the same change, so the trait
is already the shape it will need.
",
    trade_offs: &[r"
The promise in stage 3 becomes implementable without touching the interface
again, and it is settled while settling it is free.

The costs, and the first is borne by somebody who is not here.

**A provider wrapping something that is not `Sync`** — a C library handle, a
client with interior mutability — has to do its own locking. That is a real cost,
paid by the provider author, who has the least say in it. It is still the smaller
cost: the alternative charges *every* provider author for async, whether their
provider needs concurrency or not.

**A lock where there was none.** `Mutex` in place of `RefCell` on a path that is
not contended: a few nanoseconds and one more way to deadlock in code that
currently cannot.

**Threads carry stacks.** A pool of fifty is nothing on a large machine and is
not nothing on a small one, so the bound becomes a number somebody has to tune,
and a number somebody has to tune is a number somebody will get wrong.

**And it is a bet.** If a workload appears that genuinely needs tens of thousands
of concurrent conversations, this is the wrong answer and reversing it means
changing the trait after all — the exact thing this record exists to avoid doing
late. The fleet sizes this tool is built for are the reason to take the bet.
"],
    constraints: &[],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: slipway_core::date!(2026, 9, 6),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[],
);
