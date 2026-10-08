use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "A machine's identity is the grant the fleet issued it",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    subsystems: &[taxon!(Subsystem, Cli)],
    context: r"## Context

0009 says a run carries an actor identity — a person over OIDC or Teleport, CI
over workload identity, an agent over a grant it was issued. The grant is now
real (`kind=…;target=…;op=…`) and the engine refuses a change that reaches
outside it, but the grant still arrives as a command-line flag: whoever runs the
tool types what they are allowed to touch. That is a person's posture, not an
agent's.

An agent is a machine. It does not decide its own boundary — it is handed one
and cannot argue with it (0012). A grant typed on the command line is a
boundary the thing being limited wrote for itself, which is no boundary at all.
",
    decision: r"
The fleet names the grants it issued: a `[grants]` table mapping an identity to
its grant string. A run whose `--as` names an identity the fleet granted carries
that grant, and the engine checks it as it checks any other. A run that names
such an identity and also gives `--as-scope` is refused: the grant was issued,
not typed, and a second grant beside the issued one is a contradiction the run
must not resolve silently.

An identity the fleet does not grant — a person or a CI job — still carries
`--as-scope`, exactly as before. The fleet is the one document that already
names who may sign (`[approval]`), so it is also where who may touch what is
written down.
",
    trade_offs: &[r#"
A machine's grant is a fact of the deployment rather than an argument of the
run, so the agent cannot widen itself by changing what it types, and two runs
of the same agent are limited identically.

The cost is that an agent needs its grant written into the fleet before it can
act — a machine with no issued grant is refused rather than defaulting to
touching everything. That is the point: an unissued grant is not "everything",
it is nothing.
"#],
    constraints: &[
        "The grant string parses with the same `Grant` parser `--as-scope` uses; a malformed grant in the fleet is refused where the fleet is read.",
        "A run that names a fleet-granted identity and also gives `--as-scope` is refused, naming both.",
    ],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 10, 5),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[],
    introduces_norm: false,
);
