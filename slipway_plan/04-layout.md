# 04. Layout, delivery, lints, gate and CI

## The repository during the pilot

```text
nerpa/
├── crates/            the product; its shape does not change
├── xtask/             nerpa's gate, taught to read the registry in wave 1
├── doc/               the Slipway registry of nerpa (L1)
│   ├── Cargo.toml     publish = false
│   ├── lib.rs  build.rs  taxonomy.rs
│   ├── adr/  rfc/  thrust/  slice/  work/  journal/       Slipway's types
│   ├── proposal/  review/  invariant/  observation/       local types, by wave
│   └── body/          Markdown bodies the records include
├── docs/              guides and overview; decisions/ and rfcs/ empty as waves land
└── slipway_plan/      this plan, emptied into the registry and then removed
```

**One registry for the pilot and the product.** `cargo slipway` resolves a
`Slipway-Work: wNNNN` trailer in one registry, and two registries in one repository
would make every trailer ambiguous. Slipway itself keeps its methodology work in the
registry that holds its product work, under a subsystem of its own. nerpa does the
same: the pilot is the thrust `slipway_pilot`, under the subsystem `Pilot`
([05](05-sequence.md)).

## L1 — decided: `doc/`

The registry sits where `cargo slipway` looks for it — `doc/taxonomy.rs`,
`doc/work`, `doc/slice`, `doc/journal`, `doc/Cargo.toml`
([layout.rs](https://github.com/anishukserg/slipway/blob/master/crates/slipway-cli/src/layout.rs))
— so the tool works as shipped, and Slipway's layout needs no change before nerpa can
use it.

- **Costs:** `doc/`, the compiled registry, sits beside `docs/`, the guides and the
  overview. The two answer different questions, and `docs/DOCUMENTATION.md` and
  `docs/MAP.md` say which.
- **The name:** `slipway_pilot` survives as the name of the pilot's thrust. Its
  subsystem is `Pilot`, because a commit scope admits `a-z` and `0-9` only.
- **Refused:** a symbolic link from any other name to `doc`. The gate reads journal
  events through `git ls-tree`, would find one link where the files should be, and
  would pass having checked nothing.
- **Not filed as a blocker:** F5, the fixed root. If `doc/` beside `docs/` proves a
  friction, that is an observation of the pilot, not a failure of Slipway.

Markdown bodies inside `doc/body/` are pages like any other, so `cargo xtask docs`
checks their links too.

## D4 — how the crates arrive

- `slipway-core`, `slipway-knowledge`, `slipway-work` and `slipway-scan` for the
  registry, and `slipway-derive` for product crates that carry anchors — all pinned
  to one revision of https://github.com/anishukserg/slipway.
- `deny.toml` gains one exact entry, `allow-git = ["https://github.com/anishukserg/slipway"]`,
  and nothing wider. Apache-2.0 is already allowed; `chrono` is new to the tree.
- `cargo-slipway` is installed with `cargo install --locked --git … --rev` at the
  same revision, in CI and on the machine that commits.
- Moving to a newer Slipway is one commit changing the revision, through the gate,
  with a work item of its own.
- Publishing to crates.io cannot be taken back, so it waits for the verdict.
- `slipway-derive` is a procedural macro and adds no code to a binary. The static
  executor is measured before and after its first anchor, and any change in its size
  is a finding.

## Lints

The registry crate takes nerpa's workspace lints, the restriction group included.
Slipway guarantees generated code only under its own profile, and says so
([GUARANTEES.md](https://github.com/anishukserg/slipway/blob/master/GUARANTEES.md)).
A probe on 2026-09-11 already passed clippy and rustdoc, warnings denied, under a
strict profile with the restriction lints on edition 2024, so F7 and F12 are dropped
([07](07-slipway-feedback.md)). Wave 0 still runs the spike with nerpa's exact
profile — an empty registry and one converted record — because that profile also
carries the `cargo` group and allowances of its own. Every allowance the registry
needs is a finding, never a quiet `allow`; a build script printing to cargo is the
one expected, and it says why. `cargo xtask lint` is extended from `crates/` and
`xtask/` to `doc/`.

## The gate

One entry point stays, `cargo xtask ci`, as nerpa's CI file argues.

- **Target:** `cargo slipway gate` runs `cargo xtask ci` as the product's step,
  surrounded by its own steps — the commit's tree, the journal, external names,
  dependency policy. It needs F1 (a doctest floor fixed at 36) and F2 (no step for a
  product) from Slipway.
- **Until then:** `cargo xtask ci` and `cargo slipway msg-check` hold the rules, and
  there is no `pre-commit` hook. `landed` needs a proof that only the gate records, so
  no work item is started before it can also land: work done earlier carries its
  trailer and no journal event, and is landed later by `journal import` with
  `evidence = "history"` — the tool's own, visible route for work done before proof
  existed.
- nerpa's checks that Slipway's scan makes redundant — record numbering, file names,
  a supersession's target existing — are retired only in the wave that deletes the
  Markdown carrier they check.

## CI and the branch — decided (L2)

- **Until the switch** the gate runs locally, as it does today: `cargo xtask ci` on
  the machine that commits, before a push. No commit carries a basis yet, so nothing
  Slipway promises is being claimed while the gate is local.
- **At the switch**, at the end of wave 0 ([05](05-sequence.md)):
  - the GitHub repository named in `Cargo.toml` becomes the remote the gate runs for;
  - one branch, protected, taking fast-forward pushes only;
  - `cargo xtask ci` and `cargo slipway msg-check --range` required on it;
  - every action pinned to a full commit hash, as Slipway's decision 18 does;
  - `cargo-slipway` installed by revision.
- The GitLab remote stays a copy that runs nothing, as the CI file already intends.

## Hooks

`cargo slipway hooks install` is not run while `commit-msg` belongs to xtask: it
leaves an existing hook untouched and reports success (F4, confirmed on the probe).
The hooks are written by hand, in English, in the switch commit.

Becomes: decisions on the registry's layout, on delivery and on the gate's
composition in `doc/`; findings F1, F2 and F4 in Slipway.
