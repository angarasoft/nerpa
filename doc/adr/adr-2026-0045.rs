use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "Machines apply side by side, and one recorder owns state",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    subsystems: &[taxon!(Subsystem, Core)],
    context: r"## Context

Applying cost one exchange per change, one machine after another, while reading
already visited machines side by side. 0031 said state is written from one place,
and that place was a single file rewritten after every change: at a thousand
resources a write costs 20 ms, and every machine waited for every other machine's
write.
",
    decision: r"
Machines apply side by side over a bounded pool. A slot of that pool is one change
in flight on one machine, taken when that machine's next change is ready and given
back by its outcome. A change waits for the changes it comes after, not for the
rest of its stage, so a machine that never answers holds back only what waits for
it.

State becomes a directory of shards, one per machine, and one recorder owns what
it says: a writer per shard, writing in parallel, each with group commit. That
corrects 0031's serialisation point — the recorder still decides the order, and
the disk no longer does. Before a machine's next request, its shard durably holds
the record of the change before it, a mark naming the change about to be sent, and
the debts that change leaves on reactors in the same shard: one durable write for
each request.
",
    trade_offs: &[r"
The engine gains a recorder, a writer per shard, waits per edge and a scheduler
per route. State becomes a directory, format 5, and the migration is one way
except through a downgrade command. What state can miss when a run is interrupted
grows from one change to one for each machine the pool is sending to. A request
that never answers holds its machine's slot until a stop ends the wait, which is
step 1's accepted cost.
"],
    constraints: &[],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 17),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[],
    introduces_norm: false,
);
