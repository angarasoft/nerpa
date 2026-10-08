use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "A privileged executor is one the machine already had",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    subsystems: &[taxon!(Subsystem, Executor)],
    context: r#"- Corrected: 2026-09-10 — the example named `/usr/local/libexec`, and `install`
  does not create a parent that is not there. A stock Gentoo has no `libexec`
  under `/usr/local`, so the command in this record failed on the first machine
  it was run against. `/usr/local/sbin` exists, is root's, is on root's path, and
  says what it is: a program not meant for ordinary users to invoke.
- Corrected: 2026-09-11 — "Now the rule means what it says. That user may run that
  one program as root and cannot change what it is." The second half holds and
  the first does not. The executor runs whatever process it is asked to (0033),
  so a rule that lets an account run it as root is root for that account, stated
  narrowly. What an installed executor buys is that the account cannot swap the
  binary for another; it does not narrow what the binary will do. Found by the
  review of RFC 0001.
## Context

[0035](0035-privilege-belongs-to-the-transport.md) put escalation in the
transport: the fleet says `become = "sudo -n"`, and the whole executor runs
privileged rather than each operation asking for itself. That decision said
where privilege is arranged and did not say what it is arranged *on*.

Separately, delivery was built. Nerpa copies its executor to
`.cache/nerpa/nerpa-executor-<digest>` in the connecting user's own directory,
which is what makes a machine need no preparation at all.

Put together, those two produce a rule that looks careful and is not:

```
nerpa ALL=(root) NOPASSWD: /home/nerpa/.cache/nerpa/nerpa-executor-*
```

The path is narrow, the wildcard is bounded, and it grants that user root over
the whole machine — because the same user writes that path. They can put
anything there and run it as root. The rule says one binary and means every
command there is, and nobody reading it in a year will see that.

This is not a flaw in either decision. It is what the two mean together, and it
had not been written down anywhere.
"#,
    decision: r#"
**An executor that runs under escalation is one root installed, at a path the
connecting user cannot write.** Nerpa's own delivery is for unprivileged work
and for machines where that user having root is already true and already
intended.

Two ways to run, and they are not interchangeable:

**Delivered.** No preparation on the machine. The executor arrives in the user's
directory named by a digest of itself. Fine for reading, for files that user
owns, and for a machine created to be broken.

**Installed.** Once, as root:

```console
# install -o root -g root -m 0755 nerpa-executor /usr/local/sbin/nerpa-executor
# echo 'nerpa ALL=(root) NOPASSWD: /usr/local/sbin/nerpa-executor' \
      > /etc/sudoers.d/nerpa
```

and the fleet names it:

```toml
[nodes.web-01]
ssh = "nerpa@web-01"
executor = "/usr/local/sbin/nerpa-executor"
become = "sudo -n"
```

Now the rule means what it says. That user may run that one program as root and
cannot change what it is.

**`become` without `executor` is refused when the fleet is read**, naming both
halves and the way out. It is refused rather than warned about because a warning
in a tool that prints a plan is a line nobody reads, and because the fix is one
line in a document.
"#,
    trade_offs: &[r#"
A machine that is to be managed with privilege now needs a step before nerpa can
touch it, and "no agent to install" stops being true for exactly the case people
care most about. That is the cost, it is the honest one, and it is the same step
every predecessor needs for the same reason — the difference is that this one is
one file and a sudoers line rather than an interpreter and a library tree.

Delivery does not become useless. It is what makes trying nerpa cost nothing, it
is what the two machines this was developed against use, and it is right for
every provider that does not need root.

The refusal will annoy somebody who knew exactly what they were doing on a
throwaway machine. They write `NOPASSWD: ALL` and a path of their own, which is
the same access stated plainly, and the fleet reads.

What this does not settle: how the executor gets to a machine the first time
when nobody wants to log in as root to put it there. Configuration management
bootstrapping itself is an old problem and this record does not solve it — it
only refuses to pretend that delivering into a writable directory solved it.
"#],
    constraints: &[],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 10),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[],
    introduces_norm: false,
);
