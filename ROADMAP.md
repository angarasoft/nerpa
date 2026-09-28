# Roadmap

Stages, not sprints — `0014`.
Each carries an exit criterion that can be **executed rather than judged**, and
the risk it retires. A stage ends when its criterion passes on real hardware, not
when its description feels satisfied.

Releases are tags at these boundaries. Versions stay `0.x` until stage 6, where a
minor bump may break anything.

> Reordered on 2026-09-05 by
> `0024`. Going through sixty
> properties of Terraform and Ansible showed nineteen answers of thirty against
> their mistakes and nine of thirty against their virtues — every stage here was
> about the engine, and none about the part somebody would drive. The missing work
> is now inside the stages that need it, and a stage for what cannot be changed
> later goes before the cloud.

## Stage 0 — the model and the planner ✔

Resource graph, execution domains, single-pass planner, state holding references
to secrets rather than their values, local transport. Starlark host with deferred
handles. An `os` provider covering files and directories.

Packages, services, users and sysctls moved to stage 1: all four need privileges
and a real system underneath, which contradicts "no cloud involved" and would
leave the exit criterion untestable anywhere but on one person's machine.

**Exit:** met. `nerpa apply` converges a local example from an empty machine, a
second run prints "nothing to do", `nerpa plan --json` round-trips through the
typed schema, and branching on a handle fails evaluation naming the line.

## Stage 1 — reaching machines, and something worth carrying

The executor, the `ssh` transport — through which Teleport turned out to be a
route rather than a transport of its own
(`0037`) — batch
introspection, fingerprints.
Packages, services, users and sysctls.

And the three things that make the executor worth having, which used not to be
here at all:

* **Templates** (`0023`)
  — Jinja rendered in a sandbox, from values handed over rather than ambient,
  early enough that the plan shows what a file will contain.
* **Inventory groups** — a hundred machines cannot be a hundred declarations, and
  a fleet is the thing this stage exists to reach.
* **Reacting to change** — a configuration file that never restarts anything is a
  tool that does not work.

**Risk retired:** that an identity-aware proxy is a workable transport rather than
a special case; that batching beats a round trip per task by enough to matter; and
that a configuration file can be written, rendered and acted on end to end.

**Exit:** a templated configuration file converges on three to five live nodes —
across at least two distribution families and both init systems, at least one of
them reached through a proxy that carries an identity — twice, with the second run
empty, and changing it restarts the service that reads it. The plan shows the
file's contents before anything happens. A node taken offline produces a plan that
says so and names the age of its data. Wall-clock time is recorded against the
equivalent Ansible playbook.

This clause used to read "three to five live Gentoo nodes through Teleport", and
it was changed on 2026-09-10 rather than quietly met. Three Gentoo nodes would
demonstrate one package manager and — as it turned out, since Gentoo ships both
init systems — one of those too. The version above demonstrates two of each, which
is more, and it is what the two real defects of that week required: quoting in
`/etc/os-release` was found by the second distribution and nothing else, and
`systemctl is-active` reporting an absent service as merely stopped was found by
the second init system and nothing else. Moving an exit criterion is moving a
goalpost, so it is recorded here with what it cost and what it bought.

**Measured, 2026-09-10,** against the same change written as an Ansible playbook
(ansible-core 2.19.3), from one control machine over the same routes: two
machines behind the identity-aware proxy, one behind a bastion. Seconds; medians
of six runs for cold, of three or four for kept connections.

A file rendered from a template on all three machines, as the connecting user:

| | nerpa | Ansible | Ansible, no facts |
| --- | --- | --- | --- |
| nothing to do, cold | 5.69 | 10.05 | 7.71 |
| a change, cold | 6.20 | 12.23 | 9.12 |
| nothing to do, connections kept | 1.23 | 5.73 | |
| a change, connections kept | 1.63 | 8.48 | |

`chrony.conf` rendered on the Alpine machine as root through `doas`, with
`chronyd` restarted when it changes:

| | nerpa | Ansible | Ansible, `doas` plugin |
| --- | --- | --- | --- |
| nothing to do, cold | 3.83 | 10.78 | 20.59 |
| a change, cold | 4.37 | 14.87 | 28.04 |
| nothing to do, connections kept | 1.09 | 7.20 | |
| a change, connections kept | 1.58 | 11.23 | |

Cold is a fresh `ssh` for every run: what nerpa does unless told otherwise, and
what Ansible does without a control socket left from the run before. Connections
kept is `ControlPersist` for both — Ansible through its own setting, nerpa through
an `ssh_config` that says so, because how a host is reached belongs to `ssh`. The
cold comparison alone said nerpa was twice as fast, and on three machines a kept
Ansible connection matched nerpa's cold number to the hundredth: the handshake was
most of both. Measured both ways, the difference that is left is the work each
does once it is connected. See `W-022`.

What this does not measure. Escalation on Gentoo and Rocky: those machines let
the connecting user run the executor as root and nothing else, and Ansible's
`become` needs a root shell, so the escalated comparison is one machine.
`community.general.doas`, the documented way to use `doas`, turns pipelining off
by itself, which is where most of its extra seconds go; the column headed Ansible
is the `sudo` plugin pointed at `doas -n`, which keeps pipelining. Not gathering
facts saved nothing there, because the `service` module gathers what it needs
itself.

**Met, 2026-09-10,** as the clause asks. A templated file converged on three live
machines — Rocky (`rpm`, systemd) and Gentoo (`portage`, systemd) through
Teleport, Alpine (OpenRC) through a bastion — written as root by an installed
executor, and the second run printed "nothing to do". Changing the templates
restarted `chronyd` on Alpine and `systemd-timesyncd` on Gentoo: process 17045
became 17651 and 23215 became 23319, and the empty run after that left both
alone. The plan showed the lines that change before anything happened. A machine
switched off was reported with the age of its data when `W-013` closed. The
machines were then put back by nerpa itself: the files it made removed, and a
stopped service and a configuration it had rewritten returned to what they were,
byte for byte.

## Stages 2 to 6 — in the registry

Stages 2 to 6 are now thrusts `t0002` through `t0006` in `doc/thrust/`; each
stage's exit criterion is the thrust's outcome. Stages 0 and 1 stay here as
history.

## Not scheduled

Reading a resource without taking it under management — Terraform's data sources —
and one-shot commands in the spirit of `ansible all -a uptime`. Both are real
gaps; neither blocks anything above, and the second matters more for adoption than
its size suggests.

Keeping an installed executor current without anybody logging in. Since
`0040` a new build is a
reinstall on every machine with an installed executor, and doing that by hand on
two machines was already called tedious. Both ways are wanted and neither is now:
nerpa replacing an executor of another build through that executor, shown in the
plan — it already runs as root, so the replacement adds no privilege, and it needs
a record refining 0040 — and packages, `rpm`, `ebuild`, `apk` and a tarball, so a
machine's own package manager or image carries it. The first is designed as far as
[RFC 0002](docs/rfcs/0002-replacing-an-installed-executor.md), with the findings of
three reviews still open, among them that the first executor able to replace
itself is installed by hand.
