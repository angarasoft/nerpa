use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "Teleport is a route, not a transport",
    status: DocStatus::Active,
    subsystems: &[taxon!(Subsystem, Transport)],
    context: r"- Corrected: 2026-09-06 — the two machines were named by their real hostnames
  and the proxy by its real address. What was measured is what matters and whose
  machines they were is not; a repository open to everybody is not the place to
  write down somebody's infrastructure.
## Context

[0010](0010-static-executor-pluggable-transport.md) planned a pluggable transport
and named `teleport` as one to write. The roadmap has carried it as stage-1 work
ever since, next to `ssh`, on the assumption that an identity-aware proxy is a
different way of reaching a machine and therefore a different implementation:
its own connection handling, its own certificate lifecycle, its own errors.

Two later decisions narrowed what a transport actually is.
[0032](0032-provider-logic-stays-on-the-control-machine.md) made the executor a
filter on stdin and stdout that knows nothing about how it was reached, and
[0035](0035-privilege-belongs-to-the-transport.md) made the transport a command
that produces a pipe. What is left for a transport to be is a command line.

Teleport's Machine ID writes one. `tbot start identity` produces an OpenSSH
configuration holding a `ProxyCommand` and a certificate it renews on its own,
and the certificate belongs to a bot rather than to a person — which is what
makes an unattended run possible at all. A human identity on this cluster carries
a hardware-key policy and wants a tap per operation.
",
    decision: r#"
**Teleport is reached through the `ssh` transport, and the fleet says which
configuration file describes the way.** There is no `TeleportTransport`, and
there will not be one.

```toml
[nodes.web-01]
ssh = "nerpa@web-01.example.internal"
ssh_config = "/home/somebody/.tbot/nerpa/ssh_config"
```

The document names a file rather than repeating what is in it. The port, the
jump host, the certificate and the proxy command belong to `ssh`, exactly as
they already did for a plain host; the one thing `ssh` cannot work out by itself
is that a generated configuration exists, so that is the one thing written down.

Measured before deciding, on 2026-09-06, against two Rocky Linux 9.7 machines
behind a Teleport proxy, reached as a bot identity rather than as a person:

* 64 KiB of random bytes came back byte-for-byte from both machines, the same
  SHA-256 as went in. The wire is framed CBOR — four bytes of length, then a
  body — and a proxy that touched a byte would break the frame rather than the
  content, so this was the risk worth retiring first.
* A templated file converged on both: a plan carrying a number, an apply, and a
  second run with nothing to do. Thirty-six seconds cold, delivering a 774 776
  byte executor to each; fifteen warm.
* Nothing in the engine, the protocol or the transport changed. One field in the
  fleet document did.
"#,
    trade_offs: &[r"
One transport to maintain rather than two, and everything `ssh` already does —
`ProxyJump`, the delivery path, `BatchMode`, connect timeouts — works through the
proxy without being written a second time. The next identity-aware proxy that
speaks OpenSSH costs a line in a document rather than a crate.

The reach of this tool is now bounded by what an OpenSSH client can express. A
proxy offering no ssh-compatible entry would need a real transport after all, and
this record is what such a proposal has to argue against.

The certificate's lifetime is outside the tool. `tbot` renews on its own, and if
it stops, machines become unreachable — correctly reported, by
[0020](0020-a-machine-that-cannot-be-reached.md), but reported as a machine that
did not answer rather than as an identity that expired. Whether a run outliving a
certificate survives on connections already open is untested and is written down
as such; nothing here has run for an hour.

A fleet naming a path is a fleet tied to one control machine's filesystem. That
is the same tie `executor` already carries and it is not new, but it is now on
the path everybody uses rather than on an override.

What this does not settle: session recording, per-session MFA, and whether a
Teleport role can express what a provider needs to do. Those are the cluster's
business, they are decided by people who do not read this repository, and no
decision here can make them true.
"],
    constraints: &[],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 6),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[],
);
