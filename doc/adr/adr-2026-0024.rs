use slipway_core::nonempty_str;
use slipway_knowledge::{Breaking, DocStatus};

slipway_knowledge::adr!(
    title: "Plan for their virtues, not only against their mistakes",
    status: DocStatus::Active,
    subsystems: &[],
    context: r"## Context

Sixty properties of Terraform and Ansible were gone through — fifteen strengths
and fifteen weaknesses each — and each one checked against what this project has,
has decided, or has nothing for. The counting was the point.

| Against | Built | Decided | Nothing |
| --- | --- | --- | --- |
| Terraform's weaknesses | 9 | 4 | 2 |
| Ansible's weaknesses | 10 | 4 | 1 |
| Terraform's strengths | 6 | 5 | 4 |
| **Ansible's strengths** | **3** | 5 | **7** |

Nineteen answers of thirty against their mistakes; nine of thirty against their
virtues.

The imbalance is not an accident of effort. **Not repeating a mistake is a
decision, and matching a virtue is work** — usually unglamorous work like
templating, inventory and packaging. A process that writes the decision before
the code produces more of the first by construction, and this one has been
running that way for twenty-three records.

Stated plainly: the engine has come out well and the part somebody would
actually drive has not been built. The roadmap did not show this, because every
one of its stages was about the engine.
",
    decision: r"
The roadmap gains the missing work **inside the stages that need it**, rather than
beside them as a second track. A parallel track drifts, and the whole point is
that these things are not optional extras.

**Stage 1 takes templates, inventory groups, and reacting to change**, alongside
the executor. Without them the executor carries nothing worth carrying: no
templates means nothing to write but literals, no groups means a fleet is a
hundred lines of declaration, and no reaction to change means a configuration
file that never restarts anything.

**A new stage goes before the cloud, for what cannot be changed later.** Secrets,
because the architecture claims state holds no secret values and today it would
hold one in clear text; renaming a resource, because addresses are frozen by
`0002` and there is no path from one name to another; and facts as values, which
needs its boundary drawn by a decision rather than discovered in an
implementation.

Everything else moves down by one and keeps its exit criterion.
",
    trade_offs: &[r"
The plan now reflects what the tool has to be able to do, not only what it has to
avoid. And the imbalance is visible where it can be acted on rather than in
somebody's impression six months from now.

Two costs. The cloud arrives later than it would have, which is the deliberate
part: a state format and an address scheme are cheap to change today and are
migrations once anybody's state file exists. And the new stage has no external
proof — nothing about it can be demonstrated on a fleet, so it will feel like
delay while being the opposite.

The habit is worth keeping more than the conclusion is. **A count of what a
project answers against its predecessors is an hour of work that changed the
plan**, and it should be repeated at the far side of the cloud stage — not
because the list of properties will change, but because the weight of each will:
what looks like convenience at five machines is a condition of working at two
hundred.
"],
    constraints: &[],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: slipway_core::date!(2026, 9, 5),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[],
);
