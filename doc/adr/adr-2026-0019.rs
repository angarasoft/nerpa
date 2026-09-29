use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "Call it nerpa",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    subsystems: &[taxon!(Subsystem, Cli)],
    context: r"## Context

The project was called `sarma` for its first fourteen commits, after the squall
that comes off the Sarma valley onto Baikal. The reasoning was speed.

Two things were wrong with it, and neither was visible until the architecture had
been written down.

Speed is not what this tool is for. Section 01 says so in as many words: Rust's
performance is a consequence, and the thing being sold is one graph spanning the
cloud and the machines in it, plus a trust model. A name about force describes
the wrong property.

And the sarma is the wind that capsizes boats and drowns people. Naming a tool
that changes production after a sudden destructive squall is a joke that lands
badly the first time it breaks something — and worse on a package manager that
runs unattended on a timer.
",
    decision: r"
The infrastructure tool is **nerpa**, after the Baikal seal: the only animal that
can reach any point of the lake at any time of year. That is the thesis of
`0001` in one sentence — reach across domains, under one graph — rather than a
claim about how fast it gets there. It is also endemic and self-sufficient in a
hostile place, which is on message for a tool meant to work in closed networks.

The Gentoo package manager that held the name becomes **barguzin**, the steady
wind off the eastern shore that fills sails. A tool that keeps a fleet's packages
where they should be, quietly, on a timer, is the opposite of a squall.
",
    trade_offs: &[r"
Both names were free on crates.io the day this was written, and nothing had been
published under either. The Ansible role for the package manager had never been
wired into a playbook, so the swap cost a tarball, an ebuild and a directory
rename rather than a migration.

The cost is that this was the last cheap moment and it nearly passed. `0002` says
addresses freeze once artifacts exist; the same is true of project names, and it
is worth noticing that the argument which settled this one only became visible
after the architecture was written. Naming before the thesis is clear produces
names about the wrong property.

A residual risk stays open: crates.io does not let a name be reserved, only
published, and publishing means opening the source before `ROADMAP` says it
should. The name is unguarded until then.
"],
    constraints: &[],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 5),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[],
);
