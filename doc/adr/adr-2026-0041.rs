use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "A premise is sealed for the machine it describes",
    status: DocStatus::Active,
    subsystems: &[taxon!(Subsystem, Core)],
    context: r#"## Context

[0034](0034-a-plan-carries-its-premise.md) made a plan carry the premise it was
computed from — a digest of every observed value — so the executor can look again
before acting. It left one thing open in so many words: a secret's digest is keyed
by the state file and the executor holds no key, so either the premise digests
secret attributes unkeyed, "more honest, and the bookkeeping is easy to get
wrong", or secrets stay out of the premise.

The code had taken the first way without saying so. Every observation went into
the premise under an unkeyed digest, secret or not, and a `Change` serialises
whole, so `nerpa plan --json` wrote it out. A test with a secret file already on
disk and a new value in the store found the unkeyed digest of the old credential
in the plan's JSON. For a short secret that is a lookup rather than a one-way
function, which is the reason `secret.rs` keys its own digests. No secret store
was real yet, so no plan made by the command line could have held one; the first
store would have.

## Alternatives

**Keep secrets out of the premise.** No protocol change and nothing to reinstall.
It gives up the check 0034 exists for on exactly the files where writing over
somebody else's change is worst, and it is thrown away the day this is built.

**Send the state file's key.** Simpler, and a machine holding it could test
guesses against every secret's digest in state, not only its own premise. A
derived seal costs one function call.

**Encrypt the plan.** OpenTofu's answer for state and plans. It hides everything a
person reads a plan for, to protect the one field nobody reads.
"#,
    decision: r"
**The planner seals every premise digest for the machine it describes, the engine
puts that machine's seal on the batch as it leaves, and the executor refuses a
premise that arrives without one.**

The seal is derived rather than handed over: `blake3::derive_key` over the state
file's key and the machine's name. The plan carries sealed digests and no seal;
`Perform` carries the seal and is not written down. The executor seals what it
reads again and compares.

In the same protocol version, `Observation::Sealed`: the far end answers with the
sealed digest of what another observation found, and never with the thing.
Nothing asks for one yet. It is here because comparing a password hash or a secret
file without the engine holding it needs exactly this, and a protocol version is a
reinstall on every machine with an installed executor.
",
    trade_offs: &[r"
The protocol is version 4, and every installed executor is reinstalled. That is
the cost this was chosen knowing, and the reason the sealed observation came in
the same version rather than a later one.

A plan's premise can be checked only with the state file that made it. A plan
could never be applied without that state, so nothing that worked stops working.
Premise digests for the same machine differ between state files; plan numbers
never included the premise and are unchanged.

A machine can test guesses against its own premise with its own seal. It can read
its own disk, so this gives it nothing it did not have.

Unkeyed digests still exist in memory, between reading and planning. They are not
serialised, and the test that found this looks for them in the JSON rather than
trusting that.
"],
    constraints: &[],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 10),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[crate::rfc::rfc_2026_003],
);
