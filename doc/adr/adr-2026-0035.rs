use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "Privilege belongs to the transport, and no password travels",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    subsystems: &[taxon!(Subsystem, Transport)],
    context: r"## Context

Packages, services, users and sysctls need privilege. Ansible answers this with
`become`, and the answer is a matrix: `sudo`, `su`, `doas`, `pbrun`, `pfexec`,
`dzdo`, `ksu`, `runas`, `machinectl` and more, each with its own argv, its own
notion of success, and its own prompt — with `become_user`, `become_flags`,
`become_exe` and `become_pass` on top.

The matrix is that size because escalation lives *inside the task layer*. Every
task escalates for itself, so every method's quirks surface where the tasks are.
The most fragile part is the last one: a password is written to the far end and a
regular expression watches for the prompt that asks for it.

We have a choice about where to put it, because
[0032](0032-provider-logic-stays-on-the-control-machine.md) made the executor a
filter on stdin and stdout that knows nothing about how it was reached.
",
    decision: r#"
**Escalation is the transport's business.** The engine asks to reach a machine;
how the far end comes to be privileged is settled before the first byte of
protocol is written.

```text
ssh web-01 sudo -n /usr/local/bin/nerpa-executor
```

No pty, no prompt, no regular expression, and — the point — **nothing about
privilege in the wire protocol**. The matrix does not disappear; it moves to
where "how do I get onto this machine" already lives, which is the node in the
inventory. Unlike the protocol, that can grow without breaking anybody.

**No password travels.** `sudo -n`, `doas -n`, or already being root. A sudo
password is a key, and [0009](0009-carry-identity-not-keys.md) is about not
carrying keys; a run that needs one fails saying so, and the fix is a line in
`sudoers` or a different identity. That is an operational change somebody makes
once, rather than a secret this tool holds every time.

Somebody will ask for the password anyway, with a machine they cannot change and
a deadline. Refusing then will look like pedantry, which is why it is written
down now.
"#,
    trade_offs: &[r#"
The wire says nothing about privilege, so nothing about privilege can freeze into
it. One escalation per machine per run rather than one per task, which is also
one fewer thing to go wrong two hundred times.

The costs.

**Everything the executor does is privileged**, including the operations that did
not need to be. Per-operation escalation would be finer, and it would put the
matrix in the protocol; what limits the authority instead is the plan the batch
came from ([0033](0033-process-is-a-providers-word-not-an-authors.md)) and the
premise it was computed against
([0034](0034-a-plan-carries-its-premise.md)).

**A machine whose `sudo` demands a password cannot be managed** until somebody
changes that. This is a real refusal to a real request, and it will be met at
least once by somebody who cannot change the machine.

**`requiretty` still exists.** A `sudoers` that insists on a tty refuses
`sudo -n` down a pipe. It is rare and it is fixable, but the failure will read as
"the transport is broken" long before it reads as what it is, so the message has
to say which.
"#],
    constraints: &[],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 6),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[],
    introduces_norm: false,
);
