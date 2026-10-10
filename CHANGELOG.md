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

- Decision records in `doc/adr/`, immutable once accepted and superseded
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
  borrowed from (`0030`):
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

- A fleet, as a document: `--fleet fleet.toml` says which machines there are and
  what is written down about them, and `for host in group("web")` builds a graph
  for all of them from one declaration. Values come from groups and a node may
  override them; two groups giving one node the same key is an error naming both
  rather than a precedence resolution, because "where did this value come from"
  should have one answer per value instead of a table of twenty-two levels.

  Brackets for what somebody wrote down and a dot for what the machine will say:
  `host["workers"]` is settled before evaluation and may be compared, iterated and
  branched on, while `host.os_family` may not. That difference is the whole reason
  a group can decide which resources exist and a fact cannot.

- `ssh`, and the first machine that was not this one. A node says how it is
  reached — `ssh`, where the executor is, and what to run it under — and nothing
  else does. Shown end to end against a Rocky Linux machine through a bastion:
  a group iterated, a template rendered, a plan showing the file's contents, CBOR
  over one connection, the file written, a second run with nothing to do, a
  hand-edited file reported as drift, and `os_family` read from the machine
  choosing the path. Then the same with `become = "sudo -n"`, writing a file as
  root with mode 0600 — no password anywhere, no pty, no prompt to match.

- The executor gets itself onto a machine. It is copied under a name carrying a
  digest of the binary, so a machine holding a different build holds it under a
  different name and the two do not fight, and "is the right one there" is a file
  test rather than a version negotiation. Written beside and moved into place,
  because a half-copied file that is executable is worse than no file. Measured
  against two machines behind a bastion: 25 s the first time, 10 s the second.

- Teleport reaches machines, and needed no transport to do it. `tbot` writes an
  OpenSSH configuration holding a `ProxyCommand` and a certificate that renews
  itself, so a fleet names that file — `ssh_config` — and the `ssh` transport
  does the rest. Shown against two Rocky 9.7 machines as a bot identity: 64 KiB
  of random bytes survive the proxy byte-for-byte, a templated file converges on
  both, and the second run is empty. One field in a document, no code, and one
  stage-1 item that turns out not to exist —
  `0037`.

- What a repository needs before anybody outside can look at it: `SECURITY.md`,
  `CODE_OF_CONDUCT.md`, and issue forms that ask for the one thing each kind of
  report needs — the plan output and whether the second run was empty, the
  identifier of a guarantee that did not hold, or an observation that commits
  nobody to anything.

- The security policy is checked against the invariant registry, in both
  directions. It tells a researcher which guarantees are believed rather than
  demonstrated so that reporting one is not mistaken for news, and that list
  would otherwise go stale the first time an entry was promoted — telling
  somebody a weakness is known about something now claimed to be proven.

- Generated values attack the two claims a plan number rests on: that the
  canonical encoding is injective, and that a value read back from the state
  file is the value that was written. `proptest`, four properties, and an alphabet of
  four strings, four integers and four keys — chosen small on purpose, because
  injectivity is broken by two values differing in one place and two arbitrary
  values never are. Measured: with arbitrary strings the same properties stayed
  green against an encoder that wrote a boolean's tag and dropped its
  value.

- The step that opens a connection to each machine takes the opening as a
  parameter, so a test can fail it. It was the one step no test could reach, and
  it was where a fleet with a machine switched off hung — every guarantee about
  unreachability lives below it and never got asked. `W-013` closes.

- Two invariants said `enforced` for guarantees nothing holds.
  `INV-PREMISE-001` is refused by the executor and no planner produces a premise
  for it to refuse, so every run sends an empty one and nothing is checked
  between looking and acting. `INV-FACT-001` said `enforced` while its own
  `bypass` field described the hole — and `INV-PLAN-001`, which is the identical
  gap for a handle, was honestly `claimed` the whole time. Both are `claimed`
  now and both are published in `SECURITY.md`, which is where this project says
  its thin places are.

- The fact-gathering loop turns every failure into unreachability, including a
  permission error and a protocol violation, which is what `INV-REACH-002`'s
  bypass forbids in as many words. It was a deliberate choice — the subject
  there is the machine rather than a resource on it — and it was never written
  down. The entry names it now, along with what it costs: an executor answering
  a look with the wrong thing reads as a machine that is down.

- Three `// ENFORCES:` anchors sat on functions that enforce something else.
  The gate checks that a named file contains the anchor and cannot check that it
  is in the right place, which is the caveat `RULES.md` already states.

- A mode written as a number is refused rather than read as a decimal, which
  narrows what the change before it accepted. Text was read as octal and a number as
  itself: `mode = 400` landed as `0620`, group writable, where `"400"` is `0400`,
  and Starlark evaluates `0o600` to 384 before nerpa sees either, so the two cannot
  be told apart. Found by the narrow review of RFC 0001's sixth revision.

- Delivery sweeps what an attempt that was killed left beside the executor's name.
  A far side that is killed never reaches the removal at the end of the copy, and
  delivery is retried, so each attempt left another part of a binary behind for
  good. Found by the narrow review of RFC 0001's sixth revision.

- Delivery reads what the far side says while it writes the executor to it. It
  wrote all eight hundred kilobytes and only then read, so a far side that filled a
  pipe with what it said stopped reading and both sides waited for ever, holding a
  worker of the pool with no deadline and nothing to kill. Found by the narrow
  review of RFC 0001's sixth revision.

- A plan with nothing out of reach, and something waiting, says which it is where
  its number would be. The refusal counted only what was out of reach, so such a
  plan said it could not look at none of its machines. Found by the narrow review of
  RFC 0001's sixth revision.

- A held removal names the root of the wait however many steps away it is. Only a
  resource waiting for it directly was passed over, so a plan could name a chain
  that led back to the removal and never name the machine that did not answer.
  Found by the narrow review of RFC 0001's sixth revision.

- An executor cut short in delivery is never taken for a whole one. The copy was
  `cat`, which takes an input ending early for the end of the file, so an `ssh`
  killed part way left a short executor under its final name; the file test that
  decides whether to deliver passed on every run after, and the short binary died
  without a word. The copy is counted before it is moved, and so is what is already
  there. Found by the review of RFC 0001's fifth revision.

- A held removal is said to wait for its root. It named the first thing recorded
  after it that could not be planned, which could be a resource waiting only for
  that removal, so the plan said each waited for the other. Found by the review of
  RFC 0001's fifth revision.

- A mode written as a number, as Starlark's `0o600` is, is the mode the file gets.
  It was neither refused nor applied: the planner checked only text, and the os
  provider read only text and fell back to its default, so every run changed it
  again. Found by the review of RFC 0001's fifth revision.

- A file declared where another moves away from is made after the move. The name
  a move leaves was not taken, so the file was read there: adopted from the file
  about to be deleted, which the move then deleted, or updated so that the move's
  own premise moved. Either took more runs to come back. Found by the review of RFC
  0001's fifth revision.

- Only what can move out of a thing goes before its removal. The one order of
  684c024 put a removal, or a replacement's removal of what was there, after the
  change of anything state recorded after it: a moved configuration file and the
  service reacting to it were refused as a cycle, a removal nothing was inside went
  after an update, and a replaced cause was held where an update owed its reaction.
  Now only a declared resource on the same machine, of a kind its provider names and
  declared under another name than state recorded, goes first or holds. Found by
  the review of RFC 0001's fifth revision.

- What waits for something that cannot be planned is listed as waiting, with what
  it waits for, and only a machine that did not answer is reported as one. A change
  held on a machine that answered was reported as that machine not answering, and
  a held change on no machine was not reported at all. `Plan::waiting` and
  `BlastRadius::waiting`; a plan with either list is incomplete. Found by the
  review of RFC 0001's fourth revision.

- A file mode is compared as the number it spells, and one no file can have is
  refused while planning. Written "644" or "0o644", a mode never matched the "0644"
  a reading reports, and "100644" never matched at all, because a machine keeps
  only the permission bits: each was an update on every run. Providers say which
  attributes are modes (`Schema::mode`). Found by the review of RFC 0001's fourth
  revision.

- A read interrupted by a signal inside a frame carries on. The prefix was read
  with `read_exact`, which asks again, and the body turned the interruption into a
  lost connection, so a signal handler installed without restarting reads would
  have lost a machine in the middle of its answer. Found by the review of RFC
  0001's fourth revision.

- A recorded file moved onto the path a removal frees is replaced, which removes
  the old one, rather than made again beside it. Naming what the removal takes away
  dropped the file's reading, though it had been taken where state left the file,
  so the move was planned as a creation: the old path stayed on the machine with
  nothing recording it, and the next run had nothing to do. Found by the review of
  RFC 0001's fourth revision.

- A replacement comes after the removal, and the change, of what state recorded
  inside it, and whatever comes after moves later with it. Ordering a removal after
  the change of what was recorded after it had moved it past a replacement of the
  directory it was in: the directory was deleted with the file still inside, and
  every run refused. A directory replaced while a file recorded inside it moved out
  failed the same way. Changes and removals are now placed in one order over every
  edge, and only an order no run can follow is refused, naming each step
  (`PlanError::Unorderable`, which replaces `RemovedBetween`). Found by the review
  of RFC 0001's fourth revision.

- A resource reacting to a change, on a machine that did not answer or held
  behind one, is owed the reaction on its record before the change is sent, and a
  change whose reactor has no record waits. Nothing was owed: when the machine
  came back the file matched, nothing was changing, and the service kept the old
  file. `Change::owed_by` carries the debt, the plan shows it, and every plan's
  number changes with it. Found by the narrow review of RFC 0001; the rule was
  decided by the owner.

- A removal comes after the change of whatever state recorded after it, and waits
  while that cannot be planned. Removals came first and waited only for other
  removals, so a file moved out of a directory whose declaration went was still
  inside when the directory was removed; the removal refused, and every later run
  did the same. A removal that would have to come between a change and a resource
  naming what it takes away is refused when planning. Found by the narrow review
  of RFC 0001.

- A declared resource naming what a held removal takes away is held with it. It
  was adopted from the reading while the removal waited for a machine that did not
  answer, and the removal, planned in a later run, took the file away from under
  that record. Found while ordering removals after the narrow review of RFC 0001.

- A package cannot react. `reacts_to` on an `os:package` is refused at the line
  that says it, and when planning a graph that did not come from a configuration.
  A reaction on a package did nothing on rhel and merged it again on Portage.
  Providers say which of their kinds cannot react, and `nerpa_config::evaluate_with`
  takes the providers' schema. Decided after the third review of RFC 0001.

- A removal on a machine that did not answer is reported as out of reach and not
  planned, and so is every removal that has to wait for it. It was planned from
  state in a plan that called itself complete, and apply stopped at that first
  stage, before touching the machines that did answer. Found by the third review
  of RFC 0001.

- A service roused while its own attributes change is restarted as well as
  settled. Providers are told whether a change was roused, and `Provider::apply`
  gains `roused`. The `os` provider restarted only a reaction on its own, so a
  service enabled in the same run as its file changed was started, which does
  nothing to a running unit, and the fresh record cleared the debt. Found by the
  second review of RFC 0001.

- Being roused restarts a service only if it is running: `systemctl try-restart`
  and `rc-service --ifstarted ... restart`. A plain restart was sent, which starts
  a stopped service, while the comments and the architecture said a stopped one
  stays down; only the running case had been shown on a machine. Found by the
  second review of RFC 0001, and shown on the Gentoo test machine: a stopped,
  disabled `systemd-timesyncd` stayed stopped when its file changed.

- Renaming a resource and keeping what it names, with no `moved`, converges in one
  run. The old address's removal deleted the file and the new address was adopted
  from the reading taken before it, so the run ended with no file and a record
  saying there was one. A declared resource naming what a removal takes away is
  now created after the removal, which is what such a rename is until 0028 builds
  `moved`. Found by the second review of RFC 0001.

- A reaction is owed from before its cause is sent. The debt was written when the
  reacting resource's own stage came, so a run that stopped in between — a machine
  gone, a Ctrl-C — left the new file recorded and the restart forgotten, and the
  next plan had nothing to say about it. A reacting resource that state does not
  record yet is recorded from what was read, so that it can carry the debt. Found
  by the review of RFC 0001.

- A resource that is adopted or reordered rouses nothing. Both only write state,
  and both counted as changing, so on a first run over configured machines a
  service reacting to an adopted file would have been restarted for a file
  nobody touched. Found by a test while working out when a reaction is owed,
  before either kind had been released.

- A resource that already matches and is recorded with edges other than the ones
  it has now is reordered: the plan shows it as `[reorder  ]` with what it comes
  after, nothing is sent, and it is recorded again. An `after` written in once a
  file matched never reached state, so the file and its directory were removed
  together in any order. The first plan after upgrading reorders, once, every
  resource whose dependency already existed when it was made, because state kept
  no edge for those. See
  `0043`.

- A resource that already matches its declaration and is not in state is
  adopted: the plan shows it as `[adopt    ]`, nothing is sent to the machine,
  and it is recorded with what was read and every edge it comes after. It used
  to be planned as nothing, so it was never recorded and taking its declaration
  away removed nothing; whether a file could ever be removed depended on whether
  it matched on the first run. Removal now reaches files Nerpa did not create,
  which the plan says under every adoption. Adoptions are written to state once
  for the run. A first run over configured machines lists every declared
  resource, and plans that adopt have new numbers. Found by the review of RFC
  0001; see `0042`.

- State remembers every edge a resource came after, so removals run in the right
  order however the dependency was written. It kept only references still
  unresolved when planning: a dependency that already existed, or one written as
  `after`, was forgotten, and a file and its directory were removed in one stage
  in any order. A change now carries its edges, and they are part of the plan's
  number, so plans with dependencies have new numbers. Found by the review of
  RFC 0001.

- Two resources on one machine that name the same thing there are refused while
  planning. A provider says which attribute names the thing and in which
  namespace, and a file and a directory share `path`. Two files at one path used
  to be created one over the other, and every later plan proposed the loser
  again, so the second run was never empty. Paths are compared without repeated
  separators, `.` segments or a trailing separator; `..` and symbolic links are
  not resolved. Found by the review of RFC 0001.

- A value too large to carry to a machine in one operation is refused while
  planning. A provider declares the most an attribute may carry, and the `os`
  provider bounds a file's content at half a frame, because what is written is
  read back. A file over the frame limit used to plan cleanly and fail at apply
  with an error that named no resource, after earlier changes had been made. The
  provider checks again before sending, for a secret's value, which planning
  cannot see. Found by the review of RFC 0001.

- The `os` provider counts what a machine answers against what it asked.
  Answers are read by position and nothing checked: one too few left the last
  resource described by nothing, which reads as absent and gets it created
  again, and one too many described each resource by its neighbour. Found by the
  review of RFC 0001.

- An `executor` in the fleet has to be an absolute path, with no `~`, `.` or
  `..`. The characters were checked and what they spelled was not, so
  `~/nerpa-executor` or a relative path passed alongside `become`: a binary the
  connecting user can write, run as root, which is what 0039 refuses. Found by
  the review of RFC 0001.

- A conversation with a machine that broke part way is ended, and every later
  ask to that machine says it has gone away. It used to be kept: the next ask
  would read the rest of the broken frame as its own answer, or wait for ever on
  a far end still busy with the last one. Found by the review of RFC 0001; the
  test is a far end that answers with one unreadable byte and then listens.

- A plan's premise is sealed for the machine it describes. It had carried an
  unkeyed digest of everything it read, and `plan --json` wrote it out, so a
  secret already on disk and about to be rotated left an unkeyed digest of the
  old credential in the JSON — for a short one, a lookup. Found by a test before
  any secret store was real. Digests are sealed under a key derived from the state
  file's and the machine's name; the seal goes on the batch as it leaves and never
  into the plan; an executor refuses a premise without one. The protocol is
  version 4, which is a reinstall, and it also carries a sealed observation — an
  answer that is the sealed digest of a thing and never the thing — for password
  hashes and secret files later. See
  `0041`.

- A template's last newline is kept. Jinja drops it by default and nerpa had
  inherited that, so every rendered file lost the newline its template ended with
  — and cron ignores a last line with no newline after it. Found by rendering one
  template with nerpa and with Ansible, which keeps it: each rewrote the file after
  the other ran and restarted `chronyd`, under a plan whose `was` and `becomes`
  read the same. The plan now says when a file has no newline at the end, outside
  the gutter so it cannot be read as a line of the file. It also shows what a file
  was when that was one line: sides were chosen one at a time, so a one-line file
  becoming a longer one printed as the new text under nothing at all. Every file
  nerpa rendered before this changes once, by one byte, on the next run.

- Every machine reached over `ssh` is asked which build its executor is before
  anything else, and one that names another build is left out and reported with
  both names. Found by accident while closing stage 1: all three machines ran the
  executor built on 2026-09-06 at 18:50, before three fixes — a directory that
  followed a symlink with root's `chmod`, a file created world-readable before its
  mode was narrowed, a deadlock on a large input — and every run since had been
  against it. The protocol version could not have said so, because none of the
  fixes changed a frame. The name is a digest of the running binary, so nothing
  has to be remembered to change it. The protocol is version 3, and an executor
  from before it refuses the first frame — as a request it has never heard of
  rather than as a version, measured, because it decoded the frame before reading
  the stamp. Frames are read stamp first now, so the next bump is refused as what
  it is. Machines are opened side by side, since opening waits for the handshake. See
  `0040`.

- Reading fans out over a bounded pool, one machine per thread, sixteen at once,
  answers kept in the order the work was built. Measured, and worth nothing yet
  on a fleet of two: a bare `ssh` behind the proxy takes 5.07 s and the whole run
  5.06 s, because the handshake is nearly all of it and `Piped` already spawns
  every `ssh` without waiting for one. Kept for the case it is for — machines
  doing seconds of their own work, which nothing overlaps. A test recorder that
  was thread-local read an empty list and passed once the threads arrived; it is
  held by the provider now.

- `os:service` reads and settles, on `systemd` and on OpenRC, and a reaction
  restarts. Shown on a real machine of each kind: a service stopped and taken out
  of its runlevel and put back, and `chrony.conf` rewritten with `chronyd`
  restarted — its process identifier different afterwards, and unchanged by a
  second run that had nothing to do. `systemctl is-active` is not used: for a
  service that does not exist it prints `inactive` and exits 3, exactly as for
  one merely stopped.

- A machine says which init system it runs, as the fact `init`. A question may
  now take more than one look, because this one has no single file that answers
  it: `/proc/1/comm` says `systemd` where there is one and `init` everywhere
  else, which is what sysvinit says too, so what answers is what each leaves
  under `/run`. Inferring it from the distribution would be wrong exactly where
  it matters — Gentoo ships both, and the Gentoo this was written against runs
  systemd. Read on two machines with different answers, and shown driving a
  `select`.

- A reaction that was triggered and did not complete is owed, and the next plan
  says so with the reason. The debt is written before the action is attempted
  and cleared by having got past it, so the window resolves toward reporting a
  debt that was in fact paid — one redundant restart against an outage nobody
  knows about. It survives the state file, which is the whole of being a debt.
  Shown end to end: a run that could not write the file left `owed`, the next
  plan raised it although nothing had changed, and paying it cleared it. The
  state format is version 4, and an older nerpa refuses a newer file rather than
  dropping a debt it does not understand.

- `os:package` installs and removes, shown on a real machine through an
  identity-aware proxy with an installed executor under `sudo -n`: `app-misc/jq`
  installed, read back off the machine rather than echoed, a second run empty,
  the declaration removed and the package gone. `--oneshot` keeps it out of
  portage's `world` set, so the declaration stays the only record of what was
  asked for.

- `become` without a named `executor` is refused when the fleet is read. Nerpa
  delivers its executor into the connecting user's own directory, so a sudoers
  rule naming that path grants that user root over the whole machine — they can
  write anything there and run it. The rule looks careful and is not, and nobody
  reading it in a year would see it. Escalation now requires an executor root
  installed where that user cannot write, which is one file and one sudoers line
  —
  `0039`.

- `os:package` reads. Two families, `rpm` with `dnf` and `portage` with
  `emerge`, chosen to stand as far apart as the space allows; a machine calling
  itself anything else is refused by name at plan time rather than sent a
  command on the chance that it works. Every command was measured against a real
  machine before it was written down — `qlist` was rejected as the wrong
  instrument, because it matches by substring and answers `dev-lang/python-exec`
  when asked about `dev-lang/python`. See
  `0038`.

- `os-release` values in single quotes were read with the quotes attached, so a
  Gentoo machine reported a family called `'gentoo'` and was refused as unknown.
  Rocky writes `ID="rocky"` and Gentoo writes `ID='gentoo'`; the specification
  permits both. Found by reading a second distribution, which is the whole reason
  there are two, and by nothing else — the parser had no test at all.

- A name and a slot may hold a `/`, and a machine may not. In portage the
  identity of a package is `category/name` — three different packages are called
  `nginx` on a stock tree, and `portageq` refuses a bare name as an invalid atom
  — so an address that could not hold the separator could not name the thing,
  and two declarations would have collided in the graph over a name neither of
  them owns. A widening, so nothing that parsed before stops parsing.

- A machine's facts are gathered before its resources are read, and handed to
  the provider that reads them. A provider deciding how to ask a machine needs
  to know what the machine is — whether packages arrive by `dnf` or by `emerge`
  is read, not guessed — and the engine already gathers that once per machine,
  so a provider asking for itself would be the second journey `INV-BATCH-001`
  exists to prevent. A machine that will not answer is also now discovered
  before a journey is spent on it rather than after.

- A resource can say what it reacts to, and the plan says what will be roused and
  by what before anything happens. `reacts_to = [conf]` is an edge declared by
  reference: a typo is an evaluation error naming the line rather than the
  silence a global handler table gives, the restart is ordered after the file
  rather than piled at the end of a run, and a reader scanning a fleet-wide
  change sees the bounce coming. Not built yet: the debt an incomplete reaction
  should leave in state, and a provider for which being roused means more than
  applying again.

- A plan carries what it was computed from, and a machine that moved underneath
  it stops the batch instead of being written over. The protocol has carried a
  premise and the executor has checked one since both were built; nothing
  produced one, so every run sent an empty list and the check covered nothing.
  `Provider::read` now says what it looked at as well as what it found — the
  only place that can, since the engine never sees a `Seen` — and it travels
  through the plan to the batch that acts. Shown end to end: a file edited
  between the plan and the apply is not written over, and the edit is still
  there afterwards. `INV-PREMISE-001` is `enforced` again.

- One gate rather than two. The gate is `cargo xtask ci`; a CI file is a wrapper
  around it, and there were two wrappers describing each other as the mirror. It
  runs on GitHub Actions, which is where the repository lives; GitLab holds a
  copy against the day GitHub is not there, and nothing else. `.gitlab-ci.yml` is
  gone rather than kept in step — including the mutation job that was waiting for
  a runner nobody is going to install now.

- A provider says what a change is made of, and nothing about when to send it.
  `prepare` returns the operations one change needs, which of them write and what
  a write's answer confirms, and where a replacement stops being recorded;
  `conclude` turns the answers to exactly those operations into what the resource
  became; `afterwards` says whether a change that a stopped run left unrecorded
  can be read back next time or has to be sent again. The `os` provider builds
  paths, packages and services through them, so the batch it used to assemble in
  three separate places is assembled in one. `apply` and `delete` remain as the
  engine's way in, now thin: prepare, one batch, conclude. RFC 0001 §5.2.

- The engine sends what a provider prepared, and no provider is handed a
  connection to send it with. One batch for a change, and two where a replacement
  has to drop its record in between: the half that removes goes with the change's
  premises, the record is forgotten and written down, and only then does the half
  that makes it go. What comes back is counted against what was asked, because
  answers are read by position and one too few describes the last operation by
  nothing. A change is sent to the machine its resource lives on, so a resource
  in the `control` domain — which has no machine — is refused by name rather than
  sent nowhere.

- `Provider` has no `apply` and no `delete`. A change is `prepare`, a batch the
  engine sends, and `conclude`; the only connection left on the trait is the one
  `read` needs, and `INV-PROVIDER-002` records what that is worth and how it
  could still be got around.

- A secret is carried by the write that sets it, or the change is refused before
  anything is sent. A provider says which of its operations write and what
  attributes a matching answer confirms; a secret outside all of them would be
  sent and confirmed by nothing, and state would record a digest of a value
  nobody checked had arrived — the one recording that cannot be corrected by
  looking, because looking is what a secret does not allow.

- A failure on the wire says where it stopped: before anything, at a premise, at
  an operation, or at an observation. All four were the index zero, so a batch
  refused for a missing seal read exactly like one that stopped at its first
  premise, and nothing could tell a batch that never started from one that got as
  far as its first check. The protocol version is 5 with it, which is one
  reinstall on every machine that has an executor — 0040 already refuses a machine
  carrying a build this nerpa did not ship.

- A frame is read to its end, and a key nobody defined is refused. Trailing bytes
  inside a frame were ignored — the decoder stopped where its shape was satisfied
  and never looked at the rest, so the length prefix promised more than anything
  checked — and an unknown key was skipped, which is two ends disagreeing about
  what they agreed while the version stamp says they agree. Both are refused now,
  at the top level and inside the shapes a frame carries. The version stamp is the
  deliberate exception: it reads one field out of a frame whose body this end has
  not decided to believe yet.

- A write that carries a secret says so on the wire, and is refused without a
  seal. The far end answers a marked write with a sealed digest instead of with
  what it read back — which, for a file whose contents are a password, is the
  password. The provider marks the write, because it is what knows which
  operation carries which attribute, and the engine refuses before sending a
  change whose secret is carried by an operation that does not say so: two claims
  that have to line up rather than one that is trusted. `INV-PREMISE-002` covers
  it now.

- A file write is answered with what the path hashed to, not with what is at it —
  sealed for the machine when the write carries a secret. The engine works out
  what the answer has to be before it sends, compares what comes back, and tells
  the provider that the write took rather than handing it the bytes again. A
  mismatch is a failure after the operations ran. `INV-PLAN-005` reads as the RFC
  settles it: an attribute comes back from `conclude`, or is confirmed by a
  digest the engine checked — and a secret's recorded digest comes from the
  engine's own copy of what it sent, because nothing downstream of sending holds
  one.

- A private process answers its status and a sealed digest of everything it
  printed, and a failure names neither what went into it nor what it said.
  Marking a process private withheld its input and nothing else: its output came
  back whole, and its standard error was quoted into the failure under a message
  saying the input was withheld. No operation the `os` provider builds is private
  yet, so this closes it before the first one that would.

- A machine that goes away is reported with what it said on its way out. A
  conversation let the far end's standard error through to whatever terminal the
  run had, so the report held a pipe that broke and nothing about why — and the
  case that matters is the one protocol 5 creates: an executor built for protocol
  4 refuses the first frame by complaining and exiting without an answer. It is
  drained on a thread of its own and kept to its last 64 KiB, which is the rule
  delivery already used and now shares.

- Delivery asks again when `ssh` could not connect. A shared bastion drops
  connections at random once ten are being made at once — that is what
  `MaxStartups` does, by design — and one dropped connection made a machine out
  of reach for a whole run. The three commands delivery sends are idempotent, so
  it tries three times, waiting 200 ms and doubling, with jitter taken from the
  clock so that a fleet dropped together does not come back together. What the
  far side answered is not asked again: only exit 255, which is `ssh` failing to
  connect rather than the far side speaking.

- The handshake is tried again on the same failure, since it is the very next
  connection after delivery's. Opening a machine and asking which build answered
  are one step now, because that is the part which can be started over: nothing
  but `Build` has been written and no byte of an answer has been read, so there
  is nothing to repeat. Two of the three conditions RFC 0001 §5.3 puts on it were
  not knowable before — the child's exit code was thrown away when a conversation
  ended, and a broken exchange was flattened into a string, so "it never
  connected" could not be told from "it answered and then stopped".

- A `Look` carrying a sealed observation without its seal is refused before
  anything is looked at, rather than against the observation that wanted one. The
  request was what was wrong, not the path it named, and an index pointed at the
  machine instead of at the asking.

- State can be written as a directory with a shard for each machine, so that one
  machine's outcome reaches the disk without the fleet's. `key` is written first
  because nothing can be read without it, `format` last because it says the
  directory is finished, and loading refuses any name the format does not define
  rather than reading past it. Nothing uses it yet: the marks, the entries, the
  lock and the migration from format 4 come after it.

- The state lock is held by the kernel rather than by a file existing. A run that
  was killed left its lock file behind, and every later run was refused by a run
  that no longer existed until somebody deleted the file by hand — which is the
  one thing a lock exists to stop them doing carelessly. It is `flock` now: the
  file is created once and never removed, and the lock goes when the process
  does, however it goes. A run that only reads can take it shared, which a file's
  existence cannot express.

- A version 4 state file is moved into the directory beside it. The directory is
  never put in the file's place: a directory cannot be renamed onto a file, and
  the other order leaves a moment with no state at all, which loads as a first
  run with a fresh key and nothing recorded. So `<state>.d/` is built, the old
  file is kept as `<state>.v4` for a downgrade, and what is left at `<state>` is
  something a version 4 binary refuses by its version rather than reads as empty.
  `format` records which file the directory was built from, so a migration that
  stopped part way is finished rather than begun again, and a different version 4
  file beside a finished directory is refused by name rather than guessed at.
  Both locks are taken, the file's first and the directory's second.

- Runs read and write the directory. Opening state is one step — migrate a
  version 4 file if that is what is there, take the lock, read what is in it —
  because forgetting any of the three is quiet: a run that does not migrate reads
  a first run and makes everything a second time, and one that does not lock
  writes over what another run did not know about. Planning takes the lock
  shared, so two plans may read at once and neither reads a live run's work as
  though that run had ended; applying takes it exclusive.

- A shard holds the marks of changes that were sent for its machine, beside the
  records of what they became. A mark is written before the request goes out and
  carries what the next run needs to decide what to do about it — the kind, the
  locator, the settled values that were not secrets, the edges, when it was
  written, whether a replacement's removal half is already done, what its
  provider said reading could resolve, and the entry it was written over. A
  machine with a change in flight and nothing recorded still has a shard, because
  the mark is what says that machine was being acted on. Nothing writes one yet:
  the engine comes next.

- A change is written down as being sent before it is sent, and the write that
  records what it became takes that mark away. A request sent without a mark is a
  change nothing records: the run that stops in between leaves a machine that was
  acted on and a state file saying nothing happened. The mark is written before
  the change is even prepared, so a provider that refuses to prepare has sent
  nothing and the mark goes again.

- A change that failed after its operations ran leaves an entry; one that failed
  before them leaves nothing. The far end already said which — a failure names
  where it stopped, and a stale premise means nothing was done — but every
  refusal looked the same to the engine, which flattened them all into a
  sentence. What the entry is kept for is read from what its provider said when
  the mark was written: sent again where reading cannot tell whether the change
  took, kept only to remove by where it can. An answer nobody can read by
  position counts as having begun, because it says nothing about what ran.

- A machine a run left mid-change is held. Nothing is planned on it, and the plan
  says which machine it is, since when a change there has been in flight, and how
  many of its resources went unplanned — because the point of holding it is that
  somebody has to be told. It answered, so it is not reported as a machine that
  did not: that would send a person looking at the network for something that is
  about a run. A plan with a hold is not complete and has no number, as one with
  anything out of reach has none.

- `nerpa release <machine>` says a machine is yours again after a run was killed
  while changing it, and `--control` does the same for what lives on no machine.
  A hold has to have a way out, or a killed run takes the machine with it for
  good — the trap the old lock file was. Only a person can give it: nothing here
  can tell whether those changes arrived, and somebody who can look at the
  machine can. What was in flight is not thrown away; it becomes what the next
  plan has to deal with, by what its provider said reading could resolve.

- What a failed change may have made is removed once its declaration goes. A
  change that failed after its operations ran leaves an entry and no record — a
  record is written from what the change became, and it never became anything —
  so the entry is the only thing that knows something may be out there and where
  to look for it. Removals are worked out from entries as well as records now,
  and the removal goes to the locator the entry carries. Without this it stayed
  on the machine with nothing recording it, which is the state decision 0026
  exists to make unreachable.

- A change an earlier run marked and never recorded is sent again. Where this
  run's reading matches the declaration, the declaration's own change would send
  nothing — and adopting it would write down, as a thing this run knows the
  state of, something whose provider says reading cannot settle it. So the
  entry's own change goes instead, and the plan says it is being sent again.
  Whether a change is a repeat is inside the plan's number, so approving one is
  not approving the other; every plan's number moves once for this. Where the
  reading does not match, the declaration's change goes as always. A package is
  refused by name, because installing one a second time is a command this
  provider does not have.

- An entry kept only so that what a failed change made could be removed is
  cleared by the record written from the reading. Its provider said reading
  could settle that change and the reading settled it, so the entry has its
  answer; nothing else would ever have cleared it, and it would have outlived
  the question it was asked.

- `nerpa release --entry <address>` clears one entry: a person saying they have
  looked at the machine and there is nothing there. It is the last of the four
  ways an entry goes, and the only one that is not a run's own doing — what an
  entry remembers is a change reading cannot settle, so somebody who can see the
  machine is the only one who can say. A change still in flight is refused and
  left alone, naming the smaller step that turns it into an entry first.

- Where state has no record but an entry, a resource is looked for where the
  entry says it is. What a failed change may have made is at the locator that
  change used, which is not where the declaration points once somebody has moved
  it — looked for there instead, what the change made was never seen again, and
  the plan made a second one somewhere else. The rule that a resource is found
  where it was left, not where it is now asked for, now covers the one case
  where state remembers a place without remembering a thing.

- A package and a service converge, in a test rather than in a claim. Until now
  the commands this provider builds for them were checked as far as being built:
  files had a real filesystem to converge against, and nothing ever ran a
  `dnf`, an `emerge`, a `systemctl` or an `rc-service`. There is now a machine
  in memory that keeps a package database and a service table, understands what
  this provider asks and carries out what it sends — for both families, since
  the two spell every command differently — and a command it does not recognise
  exits 127, so the tests cannot pass by agreeing with a provider that changed.

- `moved(from, to)` declares a rename, and a renamed resource keeps its
  identity. Without it a rename was a removal and a creation after it: the thing
  on the machine destroyed and made again under a new address, when nothing
  about it had changed. A rename cannot change the provider, the kind or the
  slot — that would be a different resource — and one that crosses them is
  refused naming both ends. State records the migrations a run applied, and the
  plan reports the spent ones as removable from the configuration, which is the
  part Terraform's `moved` blocks lack: they accumulate for ever
  (`0028`).

- Stages 0 through 2 are complete. What cannot be changed later is built and
  attacked: a secret never reaches the state file or the plan, a rename
  declared with `moved` keeps its identity, and a recorded configuration's
  plan cannot change without a declared change. Stage 3 — the cloud — is
  next.

- A run carries an actor identity and refuses without one (`0009`). `--as`
  says who acts; `--as-scope` and `--as-expiry` say over what and until when,
  carried now for a grant to check later. The plan and its number hold all
  three, so approving a number is approving somebody's plan, and a journal
  beside state — `nerpa.state.json.journal` — records each run for a person
  (RFC 0001 §5.9). A fleet document cannot name a key: the port, the jump
  host and the key belong to `ssh`. Every plan's number moves once for this.
  `INV-TRUST-001` moves from planned to enforced.

- The run journal is an append-only hash chain (`0009`). Every entry carries
  the entry before it, from the first entry on, so an entry rewritten,
  reordered or removed breaks the chain, and `nerpa journal verify` names the
  entry and says how much of the record stands. What a chain over itself
  cannot prove is said rather than implied: truncating the tail still
  verifies — so the count is part of the answer — and who wrote any of it is
  what the detached signature is for. `INV-LOG-001`, enforced.

- `AGENTS.md` is generated from the registry by `cargo dacc emit all` — the
  navigation, rituals and product settings an agent working here needs — and
  `cargo xtask ci` refuses a tree where it is stale.

- The registry runs on dacc 0.5.0. A divergence is a record of its own now: a
  work's `Divergence` origin points at a `Limitation` by path instead of naming
  the violated assertion in free text, so an undeclared divergence does not
  compile. The journal fold reads both shapes — the file the CLI writes and the
  directories older events sit in — so the book's work states read one history.

- One party applies and another signs (`0009`, `0067`). Where the fleet names
  who may sign — an OpenSSH `allowed_signers` file under `[approval]` —
  `apply` proceeds only over a plan a third party has signed: a detached
  signature over the plan's number, carrying the plan it approves. Whoever
  proposed the plan cannot sign it, a signer the deployment never named is
  refused, and an apply whose plan moved since the signature is refused,
  naming what moved. `nerpa sign` makes the signature with `ssh-keygen` — the
  key may live in `ssh-agent` — and nerpa reads no private key.
  `INV-TRUST-002`, enforced.

### Changed

- The registry rides on dacc 0.6.0: the six dacc crates, and the CLI the gate
  installs at the one version line `Cargo.toml` carries, move off 0.5.2. Every
  decision and specification now carries its `introduces_norm` marker (none
  declared yet — the classification into norms is a separate work item), and an
  executor admission profile exists so a landing names the executor who has the
  right.

- The registry rides on dacc 0.5.2: the six dacc crates, and the CLI the gate
  installs at the one version line `Cargo.toml` carries, move off 0.5.0.

- The invariant registry rides on dacc 0.5.0's knowledge types, retiring the
  local `Invariant` of `adr-2026-057` as that record promised. Every invariant
  names the specification whose contract it holds, the `INV-` key moves into
  the statement, reviews take `dacc_knowledge::review!`, and `doc/build.rs`
  scans and emits both registries the way it already did the plan. The six
  promises of `rfc-2026-001` become records `i0067`–`i0072` with live anchors:
  the anchor scan now reaches `doc/` and `xtask/`, so the build and the gate
  carry the sites that hold them, and the commit gate refuses a message whose
  basis names no work item of the tree.

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


- A file is opened with the mode it was declared rather than written and then
  narrowed. `fs::write` opens at `0o666` less the umask, so a file declared
  `0600` spent a moment readable by everybody, and a file's content is one of
  the two places the protocol says a secret goes.

- A fleet document could run a command on the machine doing the planning. Its
  `ssh` field went to the local `ssh` as a positional argument, and `ssh` reads a
  leading `-` as another option — `-oProxyCommand=…` runs before any machine is
  contacted, with no network and no authentication. `executor` and `become` had
  the mirror of it on the far side: `ssh` joins what trails the host into one
  string for a remote shell, so a "path" holding a `;` was two commands there.
  All three are checked where the document is read. A fleet is the artefact that
  gets swapped, shared, and one day generated from a cloud inventory.

- A process handed more input than a pipe buffer holds deadlocked the executor
  if the program said anything before it finished reading. All of it was written
  before anything drained the child. Fed from a thread now, and a program that
  exits without reading is not a failure — that is what a shell pipeline does.

- A machine that answered and then went away held a run until the operating
  system gave up. `ssh` keepalives bound that; a deadline on reading would not,
  because a batch may legitimately take minutes. What is still unbounded is a
  far end that is alive and never finishes — `W-015`.

- A replacement destroyed the old resource and could then stop without writing
  that down. `Replace` deletes before it creates; if creating failed — no quota,
  no address, no image — the run returned with state still saying where the old
  one was, and every later plan reasoned from that. The deletion is a change and
  is now recorded like one. What hid it: inside that branch `record` is a
  `Record` borrowed from state, shadowing the callback of the same name.

- A template could stop a run for ever. `pycompat` was wired in whole, which put
  around twenty-five methods inside the sandbox that the filter list never saw,
  and one of them — `str.count("")` — does not terminate. Fuel could not stop
  it: fuel is charged per instruction of the template's machine, and that loop
  runs inside a call the machine is waiting on. Measured at a full core until killed.
  Methods are a named list now, checked before the callback is reached.

- Making a directory where a symlink was gave its mode to whatever the link
  pointed at. `create_dir_all` calls a link to a directory already done, and
  `set_permissions` is `chmod`, which follows one — so a declared mode landed
  somewhere nothing declared, as root. Files had this guard and said why; the
  directory beside them did not.

- A secret could be compared against a guess, silently, by reading it back out
  of a container with `.get` instead of `[...]` — or out of a lambda, or out of
  an argument handed to a method. The comparison answered `false` without
  consulting the value, so a resource appeared in the plan if and only if the
  guess was right: an oracle, which is the thing 0027 names as the reason
  secrets are tainted at all. The same route made an unresolved attribute
  silently false. Taint now follows a method call's receiver and its arguments,
  and a lambda body is walked as a `def` body already was. Found by an
  independent review, reproduced end to end against the binary, and pinned by
  four tests written from the routes rather than the property.

- Two different plans could carry one number. An unresolved reference and a
  secret's source were hashed as their printed form, and printing joins two
  pieces with a separator either piece may contain: a resource name may hold a
  dot, so `os:file.a.b`'s `c` and `os:file.a`'s `b.c` print alike; a store path
  may hold an `@`, so path `a@b` and path `a` version `b` do too. Each piece now
  carries its own length, as values already did. Found by an independent review
  and reproduced as two failing tests before the fix. A plan holding a
  reference, a secret or a fact-bounded value gets a different number than it
  did.

- The plan document carries its renames and its merges. `migrations`, `spent`
  and `merges` join the serialized plan — the last of the three was a field
  nobody had put into the JSON — and the plan's number covers migrations and
  merges, so two plans differing only in a rename no longer share a digest, and
  approving a plan that spends a rename is not approving one that makes it.
  Every plan's number moves once for this. What a recorded configuration plans
  is now pinned by golden files in `crates/nerpa-core/tests/corpus/`, so a change
  to the plan's shape fails the build until the golden is updated and an entry
  appears here — `INV-COMPAT-002`, enforced from this change rather than
  claimed.

### Fixed

- Review v0013's nine findings, closed. The testing document names the layers
  the suite runs and what is only planned; a commit touching the corpus carries
  its changelog; a refusal names everything its number covers, or the two
  numbers it compared when words fail; `cargo xtask live --stage4` drives the
  five verbs of the stage-4 outcome, marked as needing the live park; an
  identity whose expiry passed does not run, and an expiry that is not an
  RFC3339 time is refused by name; and rule 7's every sentence has a lint
  behind it, reading past inline test modules. Beside them the convergence line
  is matched whole, and the executor installs through a temporary name of each
  run's own. Declared rather than done, and the invariants say so: an actor's
  scope, carried until grants exist.

- Documents and a test named somebody's real machines: a Teleport proxy by its
  address, two hosts by their names, a private address and a home directory.
  What was measured is what a record is for; whose machines they were is not,
  and this repository is meant to be read by people who have no business
  knowing. Examples now use the ranges and names reserved for them.

- The README showed output the tool does not produce. A file's contents have
  been printed as lines under `becomes:` since templates arrived, and a plan has
  carried a number since the premise did; the transcript still showed neither.
  It is now what the commands actually print, checked by running them.

- A fleet holding one machine that was switched off stopped the whole run, and
  stopped it by hanging: `ssh` was given no connect timeout, so it waited out the
  operating system's TCP timeout, and the failure ended the run before the
  planner could report the machine as out of reach. `INV-REACH-001` says a
  machine that did not answer is refused per resource rather than per run, and it
  was being enforced one step too late — everything the planner does was correct,
  and nothing reached it. Found by switching a machine off rather than by reading
  the code. Both halves now hold: connections are opened with a timeout, and a
  machine that cannot be opened is left out of the registry and reported by name.

- A machine that did not answer at all was told it was of the wrong
  architecture — the reason was assembled from an empty reply rather than read
  from one. `ssh` distinguishes its own failure from the far side's answer by
  exiting 255, which also means a machine that was never reached is not asked a
  second question.

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

- A declared resource was removed for being remembered by an entry. Removals are
  decided by comparing what state remembers against what the configuration still
  asks for; entries had just joined the first set and not the second, so an
  address whose only trace was an entry read as undeclared with its declaration
  right there, and one plan both removed it and made it again. Both sides ask
  `State::remembered` now, so the two cannot drift apart again.

- A machine whose last record went away kept the shard it had before. A write
  rewrites the shards of machines state has something for, and a machine that
  has nothing has no shard to write — so the file stayed exactly as it was, and
  a resource removed on the last machine it lived on came back on the next read,
  for every run after. The last thing on a machine going away now takes that
  machine's shard with it.
