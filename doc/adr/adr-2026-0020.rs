use crate::taxonomy::Subsystem;
use slipway_core::{nonempty_str, taxon};
use slipway_knowledge::{Breaking, DocStatus};

slipway_knowledge::adr!(
    title: "Refuse the resource, not the run, when a machine cannot be reached",
    status: DocStatus::Active,
    subsystems: &[taxon!(Subsystem, Transport)],
    context: r"## Context

`0006` made planning a single pass, which means every existing target has to be
read before anything happens. Machines are sometimes down.

The roadmap listed three ways out: refuse to plan at all, plan from the last
report with the age shown, or skip the node quietly. The third was never
plausible. The first two are both defensible, and that is the tell that the
question was asked at the wrong level.

Refusing the run means one machine being down blocks changes to a hundred and
ninety-nine others, which is how people learn to work around a tool. Planning
from stale data means presenting a hypothesis as a plan.
",
    decision: r#"
**The unit of refusal is the resource, not the run.**

Everything that could be read is planned. A resource whose machine could not be
reached appears in the plan, not as a change but as *nothing can be said*,
carrying the node's name and the age of the newest data there is. `apply` will
not touch it. The plan stays complete — it covers every declared resource, some
of them with an admission.

**No speculative diff.** A difference computed from data half an hour old looks
exactly like one computed from a live read: `cores 4 → 8` carries no mark saying
where it came from. Showing it would create two grades of change with different
trustworthiness and rely on everybody remembering which is in front of them. If
somebody later needs "what would happen when it returns", that is a separate
command behind an explicit flag, and by `0002` it should not exist until asked
for.

**Unreachable is not the same as broken.** A provider must distinguish "could not
contact the machine" from every other failure. Treating a permission error as
unreachability would quietly stop managing a resource because of a typo in an
access rule, which is precisely the class of silent wrongness this project exists
to avoid. Everything that is not unreachability still fails the run.

**Unreachability travels along edges.** A resource whose dependency could not be
read cannot be applied either — its values have nowhere to come from. This is the
same propagation unresolved values already have, and it needs no separate
mechanism.

**State records when each record was written**, because that is where the age
comes from. The state format goes to version 2.
"#,
    trade_offs: &[r"
The property that falls out, and the reason this shape is right rather than
merely workable: a plan containing an unreachable resource carries an age, an age
changes every second, and a hash over it therefore never settles. **Such a plan
cannot be signed** — and it should not be. Nobody can approve a change to
infrastructure they were unable to look at. `0009`'s approval refuses it by name
rather than by an unexplained hash mismatch, because an accidentally unsignable
plan is a mystery at three in the morning.

The costs are three.

A machine that stays down accumulates drift and nothing forces attention to it
beyond a line in every plan. Terraform has the same hole; naming it is better than
implying otherwise. What is available is that the count is a number rather than a
sentence, so something automated can act on it.

Time is not a hermetic value, and it now lives in state. It is confined to
reporting an age: it must never take part in a comparison during planning, or
resources would begin to differ from each other because a minute passed.

And the strict reading costs the obvious convenience. There is no way to ask what
would happen to a machine that is off without turning it on. That was weighed and
kept.
"],
    constraints: &[],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: slipway_core::date!(2026, 9, 5),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[],
);
