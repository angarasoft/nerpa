# Changelog

Notable changes, newest first. This project is pre-1.0 and interfaces change
without ceremony until the first stage is complete.

## Unreleased

### Added

- Resource addresses, with a slot that lets two versions of one declaration
  coexist and with no index anywhere in the scheme.
- Execution domains, read from an address rather than stored beside it.
- Resources: attributes, declared ordering, and whether drift on them may be
  overwritten or has to be merged by a person.
- Contract macros, on in release builds.
- The resource graph: staging by topological level, deterministic cycle
  reporting, and refusal of dangling or duplicated declarations.
- An invariant registry in `docs/invariants.toml`, checked in both directions
  against `// ENFORCES:` anchors in the code.
- `cargo-deny` configuration, with a licence allowlist that fails the build.
- `cargo xtask` as the single gate: formatting, clippy, both test configurations,
  repository rules and the invariant registry. CI runs it and nothing else.
- `docs/CODING_STANDARDS.md`, split into what the gate enforces and what review
  does.

- Decision records in `docs/decisions/`, immutable once accepted and superseded
  rather than edited. Eight of them, including the reversal that replaced lazy
  planning with a single pass.
- `docs/DOCUMENTATION.md`: which kind of documentation answers which question,
  where each lives, and what the gate checks.
- `cargo xtask docs`: decision numbering, required sections, both directions of
  every supersession, then rustdoc with warnings denied.

- Seven more decision records: zero trust, the executor and transports, observed
  state, the machine-facing surface, what is taken from Portage, how the project
  ships, and how it is tested.
- `docs/TESTING.md` and exit criteria in `ROADMAP.md` that can be executed rather
  than judged.

- Tests for the gate itself: every check now takes its input as data and is shown
  a fixture that breaks it, so no rule is trusted without having been watched
  fail.
- `cargo xtask mutants` and `cargo xtask deny`. Every tool the project uses is
  reached through `xtask`, including the ones living outside the workspace.
- Tests closing every gap the first mutation run found: the charset rule per
  alternative, `is_control` asserted both ways, `Graph::len`/`is_empty`/`get`,
  the cycle report against a graph holding resources it does not block, and the
  identifier rule one condition at a time.

- Unresolved values: an attribute holds either a settled value or a reference to
  the resource that will supply one. `Value` stays settled-only, because it is
  also what observed state is made of and an observed value is never unknown.
- An inventory saying where each node comes from, and edges derived from it: a
  resource on a node is ordered after whatever creates that node, and a resource
  on a node nobody declared is refused.
- Edges derived from references, so ordering no longer depends on remembering to
  declare it.

- `nerpa-config`: a Starlark host. `resource()` declares something that should
  exist and returns a handle; reading an attribute of it yields a value that will
  not exist until apply; `node()` says where a machine comes from.
- Three mechanisms holding one guarantee, because the toolchain allows only some
  of them to be precise. Ordering, indexing and iteration are refused by the
  value with a line. Equality and containment are refused by a pass over the
  syntax tree before evaluation, because Starlark answers them without consulting
  the value. Truthiness is recorded and the whole run refused.

- The planner: desired and observed state into a plan, in one walk of the graph.
  Creates, updates and replacements, with the attribute that forced a replacement
  named rather than left to be inferred.
- `Observed`, `Schema` and the plan types — `Plan`, `PlanStage`, `Change`,
  `AttributeDelta`, `Confidence`, `BlastRadius`. Ordered throughout, because a
  plan is going to be hashed and signed.
- References resolve against what already exists, which is what makes a second
  run empty.

- The provider boundary, and `nerpa-provider-os`: files and directories on the
  machine Nerpa runs on. Every provider must ship a fake, because the trait says
  so and a provider without one does not compile.
- `observe` and `apply`: reading the world once before planning, then walking the
  plan in the order it already fixed, filling in values an earlier stage produced.

- State: what the last run made, and what it depended on when it made it.
  Removals in reverse dependency order, taken from recorded edges because the
  configuration that described them is exactly what was deleted.
- `ChangeKind::Delete`, `Provider::delete`, and destroy-before-create for a
  replacement.
- A resource that moved is found where it was left rather than where the
  declaration now points, so changing a location is a replacement instead of a
  silent duplication.

- State on disk: JSON a person can read, written whole or not at all, behind a
  lock so two runs cannot each write what the other did not know about.
- The `nerpa` command: `plan` and `apply`, with plan output shaped after
  `emerge -pv` — fixed action column, deltas rather than values, summary before
  detail, the cause of a replacement named where it happens.
- `nerpa plan --json` prints the same object the text is rendered from.

- A machine that does not answer is refused per resource rather than per run.
  What could not be read appears in the plan saying so, with the machine and the
  age of the newest knowledge of it, and nothing is claimed about it.
- `ProviderError::Unreachable`, the only failure a run survives. Everything else
  still stops it, because everything else means something is wrong rather than
  merely out of reach.
- State records when each entry was written, and the format goes to version 2.

- Reads happen in batches, grouped by machine before anything is asked. Reading a
  machine takes one journey however many resources are on it.

- `nerpa-proto`: the wire between the engine and an executor on a machine.
  Length-prefixed CBOR frames, every one stamped with the protocol version it was
  written by, and a size that is checked before anything is allocated.

- A third kind of attribute: what a resource was born with. Editing an image
  reference no longer proposes to destroy the fleet, and the difference is
  reported on every plan rather than ignored.

- Project management, sized to this repository rather than to the one it is
  borrowed from ([0030](docs/decisions/0030-claim-less-and-attack-what-is-claimed.md)):
  `docs/MAP.md` says which kind of knowledge lives where and what has to move
  with what, `RULES.md` holds the eight prohibitions a later commit cannot
  repair, `docs/WATCHLIST.md` takes an observation for the price of a row, and
  `docs/OPEN_QUESTIONS.md` takes a question with the stage by which it is due.

- `cargo xtask budget`: what a plan costs, measured against `docs/budgets.toml`.
  A baseline refuses a regression against what the code already did; a budget
  reports the distance to what it ought to cost. Nightly, beside `mutants`, for
  the same reason.

- `cargo xtask docs` now also checks that every section of `ARCHITECTURE.md` says
  whether it is built, and that every link between written pages resolves. The
  second found five links, across four records, pointing at their own neighbours
  from the repository root, which meant none of them opened.

- A commit subject has a shape, and `.githooks/commit-msg` knows it — installed
  by `cargo xtask hooks`, checked by `cargo xtask commit-msg`, written down in
  `docs/COMMITS.md`. Twenty-five commits in, the history already spelled one type
  two ways: `[DOC]` twice and `[docs]` three times, for the same act.

- Secrets. `secret("lockbox", "prod/db/password")` names a value without reading
  it, and what comes back can be given to a resource and nothing else — it cannot
  be compared, indexed, iterated or used as a condition. State holds a reference
  and a keyed digest and has no way back to a value, so "the state file never held
  a credential" is a property of the types rather than a rule anybody remembers.
  A plan says `password: (secret) → (secret, changed)`, which is enough to
  approve and nothing more.

- Bytes. A certificate, a keytab or a file in an encoding that is not UTF-8 was
  outside the model entirely. `Value::bytes` is canonical — anything that is text
  is text — so a file holding `hello` has one representation and a second run is
  empty. JSON has no way to say "these are bytes", so a state file writes base64
  under a reserved key and refuses the one map shape that would read back as
  bytes; the wire carries a real CBOR byte string, which is the property CBOR was
  chosen for and had never been asked to do.

- Templates. `template("nginx.conf.j2", values)` renders Jinja through
  `minijinja`, so what reaches the plan is text and the plan shows the file's
  contents — as lines, not as one escaped string — before anything happens.
  Ansible's `--diff` reports during the run. The sandbox has no clock, no
  environment, no process and no filesystem beyond the configuration's own
  directory; values are handed over rather than inherited, and a value that was
  not handed over is a refusal rather than an empty string. The filter set is a
  list kept here rather than whatever the library ships.

- A plan has one number covering everything it intends to do, and a plan that
  could not look at everything has none — such a plan carries how old that
  knowledge is, and a number over a moving age never settles. It is what limits a
  batch of operations that can run anything, and what a signature will eventually
  be over.

- An executor: the program that acts on one machine, and the only thing that
  does. Bytes in on stdin, bytes out on stdout, no socket of its own — so `ssh`,
  a package already on the machine, and a pipe here are the same program. A
  static build is 767 KB.

- A transport, and a machine is a command that produces a pipe:
  `Piped::running(node, "ssh", ["web-01", "sudo", "-n", "nerpa-executor"])`. One
  connection carries a whole run, against Ansible's five or six ssh operations
  per task per host. `Here` is the executor in this process and `Held` is a
  machine that exists only in memory, which is where the fake belongs now that a
  provider has no world inside it.


- Values chosen by what a machine turns out to be. `web.os_family` is a fact and
  `select(web.os_family, {"debian": …, "gentoo": …})` picks by it, so a path that
  differs by distribution is expressible without letting a fact decide which
  resources exist. Every branch is written down, so the graph is the same
  whichever way it goes; only the value is open, and the plan prints the set
  rather than "unknown", because a person can approve a set. A machine that says
  something no branch accounts for is refused, not defaulted.

  Facts are gathered in the same journey as everything else, from a closed list —
  `arch` and `os_family` today — so what gathering costs is the length of that
  list rather than the length of what a machine could be asked.

### Changed

- Provider logic runs where the engine runs and the work happens where the
  resource is. The `os` provider no longer touches a disk: it turns readings into
  things to look at and a change into things to do, and something at the other
  end of a `Reach` opens the files. Nobody else plans centrally and runs the
  logic centrally, and the reason they cannot is that a provider deciding on a
  reading needs a round trip per decision — which a single planning pass already
  forbade.

- The wire protocol, version 2. Reading and acting are the same small closed set
  and both include running a program, because a package provider needs `qlist -I`
  to know what is installed as much as `emerge` to change it. A batch says which
  plan it came from and carries the premise that plan was computed from, checked
  before anything is done: between the looking and the doing our window is the
  whole run where Ansible's is one task.

- Privilege belongs to the transport. Ansible's `become` is a matrix because
  escalation lives inside the task layer; here it is an argument to the command
  that opens the pipe, and no password travels — `sudo -n` works or fails
  plainly, and a sudo password is a key.

- Parallelism will be threads, and `Provider` requires `Send + Sync` now, while
  there are no providers outside this repository and the change is free.

- State format 3, and state is written after every change rather than once when
  the run returns. A run that stops half way has made real changes, and recording
  them only at the end left every resource it had already made with nothing
  saying it exists. The write is now durable against a stopped machine as well as
  a stopped process, and the lock file says which process holds it.


- The invariant registry says what is proven rather than what is hoped. Every
  built entry records the `bypass` it could suffer; `enforced` now requires
  `adversarial` tests that carry that bypass out, and `claimed` is the honest
  status for a guarantee nobody has attacked. Four entries moved to `claimed`,
  each with a reproduction: `INV-PLAN-001` and `INV-SECRET-004` (a handle or a
  secret laundered through a list or a dict reaches a comparison the syntax pass
  does not match — closed by tests named after those routes, then reopened when
  an independent review found the same comparison still silent when the value
  arrives through a function parameter),
  `INV-PLAN-003` (an attribute the provider does not report back never settles,
  so no run is ever empty) and `INV-STATE-004` (the state rename is atomic
  against a stopped process, not a stopped machine). No code got worse; the count
  of demonstrated guarantees fell because it was being counted wrongly.

- `docs/ARCHITECTURE.md` marks each section `built`, `partly built` or `planned`.
  Eight of nineteen describe things nobody has built, which the present tense
  they are written in did not say.

- Renamed from `sarma` to `nerpa`, and the Gentoo package manager that held the
  name to `barguzin`. Speed was never the thing being sold; reach across domains
  is, and one of those names says so.

- Workspace lints set to their strictest: `unsafe` forbidden, `missing_docs`
  denied, clippy `pedantic` and `cargo` denied, and targeted bans on `unwrap`,
  `panic`, indexing and lossy casts. Sixty-two findings, all fixed.

- Licence is Apache-2.0. The provider ecosystem has to be written by people who
  are not the author, and nobody writes a plugin while wondering whether it has
  become a derivative work.


### Fixed

- A file the `os` provider could not read was reported as a file that was not
  there, so the plan proposed creating it and the apply overwrote it — losing its
  contents and resetting its mode. Absence and failure are different answers. In
  the same area: a symlink was described as the link's mode with its target's
  contents, a record of an object that did not exist; and removing a directory
  took everything inside it, which nothing had recorded.

- An attribute the provider did not own applied cleanly and then re-planned for
  ever. Nothing reported it back, so it never settled and no second run was ever
  empty — the tool stopped converging and said nothing about it. Reporting an
  attribute is now the provider's claim that it took it, checked for every
  provider whether its author read this repository or not.

- A value that will not exist until apply, or a secret, could reach a comparison
  by being put in a dictionary and read back out; the comparison was silently
  false and a resource silently did not exist. The check follows the taint now
  rather than matching one shape, through assignments, containers, indexing,
  iteration, comprehensions and function returns. What it still does not follow
  is a function parameter, and `INV-PLAN-001` says so.

- A configuration that could not be ordered read as a configuration that
  declared nothing: `stages()` returns a `Result` and `.flatten()` on one yields
  nothing for an error, so the run continued about the wrong thing rather than
  stopping.

- Planning was quadratic — every resource asked the graph for its dependencies
  and every one of those rebuilt every edge. Eight thousand resources took six
  seconds before anybody opened a connection, and now take 0.12.
