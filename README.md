# nerpa

<p align="center">
  <img src="assets/logo.png" alt="nerpa" width="360">
</p>

<p align="center"><b>One binary that holds cloud resources and the machines they run on in a single graph — and plans both together, in one pass.</b></p>

<p align="center">
<a href="https://github.com/nerpa-run/nerpa/actions/workflows/ci.yml"><img src="https://github.com/nerpa-run/nerpa/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
<a href="LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-blue.svg" alt="License"></a>
<a href="rust-toolchain.toml"><img src="https://img.shields.io/badge/rust-1.99.1-orange.svg" alt="Rust"></a>
</p>

nerpa plans infrastructure and configuration as one typed document, then applies
it over a transport that carries an identity rather than a static key. It is
built for a world where the operator is often not a person: a plan is a signed
artifact before it is a page of text, and a change can be made to require a
signature from somebody other than whoever proposed it.

> **Status — 0.4.0, unreleased.** `nerpa apply` converges templated files,
> packages and services across a fleet reached over `ssh`, delivering its own
> executor to each; a second run has nothing to do, and a machine that is off is
> reported by name rather than stopping the run. Cloud is in: a Yandex Cloud
> instance is created, resized and polled through its API, inside the same plan
> as the machine it becomes. See the [roadmap](ROADMAP.md).

## Example — a machine and the file on it

```console
$ cat main.star
# A VM, in the control domain. Declaring it registers the node "db".
resource("yandex:instance.db", {
    "zone_id": "ru-central1-a",
    "platform_id": "standard-v3",
    "cores": 2,
    "memory_gb": 4,
    "subnet_id": "e9b8kn2...",
    "boot_disk_image_id": "fd8...ubuntu-2404",
})

# A file on that same VM, in the target domain. It lands on "db".
resource("os:file.motd@db", {
    "path": "/etc/motd",
    "content": "managed by nerpa\n",
    "mode": "0644",
})

$ nerpa plan --as alice@example.com
  2 changes: 2 to create
  touches 1 node: db

stage 1 · control
  [create   ] yandex:instance.db
               ~ cores              — → 2
               ~ memory_gb          — → 4
               ~ zone_id            — → "ru-central1-a"
               ~ platform_id        — → "standard-v3"
               ~ subnet_id          — → "e9b8kn2..."
               ~ boot_disk_image_id — → "fd8...ubuntu-2404"

stage 2 · target: db
  [create   ] os:file.motd@db
               ~ content             becomes: managed by nerpa
               ~ mode               — → "0644"
               ~ path               — → "/etc/motd"

plan 7c1f0b2a9e04
```

One graph, one plan: the VM is created before the file, because the file on `db`
cannot exist until the node does — nerpa reads that from the graph, not from a
provisioner hook. Run it again and there is nothing to do; the world already
matches.

That number under the plan is a digest of everything the plan says and was read
from, so an unchanged world reproduces it and a moved one does not. A plan can be
proposed by one person and signed by another, who needs nothing but an ssh key:

```console
$ nerpa plan  --as alice@example.com --json > plan.json
$ nerpa sign  --as carol@example.com --with-key ~/.ssh/plan-key plan.json
$ nerpa apply --as alice@example.com --approval plan.json.approval
```

Whoever proposed a plan cannot sign it, and an apply over a plan that moved since
the signature is refused, naming what moved. Every run says who is acting
(`--as`) and lands in an append-only, hash-chained journal.

## Core ideas

- **One graph, two domains.** A resource's state is read either through an API
  (`control` — a VM, a subnet) or only by running code on one machine (`target`
  — a package, a file), and naming that difference puts a VM and the packages on
  it in one graph, with real edges between them.
- **Configuration is a Starlark program**, evaluated with no I/O, no clock and
  no network, so the same source always yields the same graph — which is what
  makes a plan worth signing.
- **The set of resources is known before anything runs.** Values that do not
  exist yet are opaque handles you can pass around but not branch on, so planning
  is one pass over the whole graph.
- **Identity, not keys.** A run carries who acts, not a static credential;
  secrets travel as references and never reach the state file or the plan.
- **The engine carries its own executor** to each machine over the transport,
  and a provider that reaches a cloud API stays synchronous and inside its crate.

Each idea has a chapter in the [book](docs/book/index.md); the contracts behind
them are in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Next to Terraform and Ansible

**Terraform** plans your cloud and stops at the machine's door: what runs
*inside* the VM is a `provisioner` nobody defends, or a second tool run by hand
afterward. nerpa's plan crosses that door — the VM and the packages, files and
services on it are one graph with real edges, so a machine and its configuration
are a single plan and a single apply. And there is no `count`, so an index cannot
shift and quietly propose destroying a live resource.

**Ansible** is the closer cousin: both are agentless, reach a fleet over `ssh`
through your own ssh config and jump hosts, and converge files, packages and
services so a second run has nothing to do. The difference is the plan. Ansible
runs an imperative task list whose `--check` can report a change it could not
actually verify; nerpa evaluates the configuration with no I/O and produces a
typed diff with a hash — an unchanged world reproduces it, and you can require a
second person to sign it. The same graph also holds the cloud resources Ansible
would reach through separate modules and a separate inventory.

It is not a Kubernetes replacement, a CMDB, or a monitoring system.

## How it is governed

nerpa's decisions, specifications, invariants and work plan are compiled Rust
code in [`doc/`](doc/): a reference to a record that no longer exists does not
compile, so the trail that explains *why* a change was made cannot rot silently.
What is proven versus merely believed lives in [SECURITY.md](SECURITY.md).

## Documentation

| What you want | Where it lives |
| --- | --- |
| Concepts, end to end | [the book](docs/book/index.md) |
| The roadmap, and what is done | [ROADMAP.md](ROADMAP.md) |
| What the system is, as contracts | [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) |
| Decisions, specs, invariants and work | [doc/](doc/) — the compiled registry |
| Which document answers which question | [docs/DOCUMENTATION.md](docs/DOCUMENTATION.md) |
| Where each kind of knowledge lives | [docs/MAP.md](docs/MAP.md) |
| How a commit is written | [docs/COMMITS.md](docs/COMMITS.md) |
| What a released version changed | [CHANGELOG.md](CHANGELOG.md) |
| How to report a vulnerability | [SECURITY.md](SECURITY.md) |
| How to contribute | [CONTRIBUTING.md](CONTRIBUTING.md) |

## Building

```console
$ cargo test
```

The toolchain is pinned in `rust-toolchain.toml`; `rustup` fetches it on the
first build.

## License

Apache-2.0 — use it, ship it, build products on it, run it as a service. The
engine stays Apache-2.0. A coordination plane for several operators at once —
grants, third-party approval, the audit chain — is planned as a separate,
separately licensed component; contributions are taken under a CLA so that split
stays possible. See [CONTRIBUTING.md](CONTRIBUTING.md).
