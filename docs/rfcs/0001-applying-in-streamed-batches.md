# RFC 0001. Applying machines in parallel, with state in shards

- Status: in review; revision 4; its fifth review logged (§9)
- Date: 2026-09-11
- Revised: 2026-09-11, after each of four reviews
- Scope: what was step 1. Replacing an installed executor is
  [RFC 0002](0002-replacing-an-installed-executor.md), and streamed batches are
  [RFC 0003](0003-streamed-batches.md). The file keeps its first name, because
  reviews and commits cite it.
- Touches: 0006, 0015, 0020, 0026, 0027, 0028, 0031, 0033, 0034, 0040, 0041, 0042,
  0043; `INV-APPLY-001` (new), `INV-BATCH-001`, `INV-PLAN-005`, `INV-PLAN-007`,
  `INV-PLAN-008`, `INV-PLAN-009`, `INV-PLAN-010`, `INV-PLAN-011`, `INV-PREMISE-002`,
  `INV-REACH-001`, `INV-REACH-002`, `INV-REACT-002`, `INV-REACT-006`,
  `INV-SECRET-*`, `INV-STATE-002`, `INV-STATE-004`, `INV-STATE-005`,
  `INV-WIRE-001`; `W-014`, `W-015`, `W-022`

## 1. Summary

Applying costs one network exchange per change, one machine after another.
Reading already costs one journey per machine, with machines side by side.

This RFC makes applying side by side too. Machines apply in parallel, each still
sending one `Perform` per change, so the protocol keeps its shape. Providers split
a change into building its operations and concluding from their answers, so the
engine, not the provider, reaches the machine. A change waits for the changes it
comes after, not for the rest of its stage. One recorder owns what state says and
writes it in a shard per machine, in parallel, with one durable write for each
`Perform`: the record of the change before, and a mark naming the change about to
be sent. The mark is what the next run knows of a change reading could not find
again. What happens when something fails is chosen per run, and a run that stops
holds the machines it was still acting on until a person releases them.

One change reaches the wire: a file write is answered with a digest of the file
read back, sealed for the machine when the content is secret, rather than with
the content, and the engine, which still holds what it sent, checks it. That is
protocol 5, and one reinstall.

Batching a machine's changes into one conversation (RFC 0003) waits until this is
built and measured.

Four reviews' findings, and where each is answered, are in §9.

## 2. What was measured

All figures are from 2026-09-10 and 2026-09-11. The fleet was Rocky and Gentoo
behind a Teleport proxy and Alpine behind an OpenSSH bastion. The control machine
was the same for every run, and the executor was the static build.

### 2.1 Where the time of one applied change goes

One exchange means one `Perform` over an open connection, measured without the
engine.

| | Rocky (Teleport) | Gentoo (Teleport) | Alpine (bastion) |
| --- | --- | --- | --- |
| Handshake | 5.0 s | 4.9–5.2 s | 3.2 s |
| One `Perform`, one 1 KiB file | 73–75 ms | 72–82 ms | 172–174 ms |
| One `Perform`, 100 such files | 444–448 ms | 442–494 ms | 775–851 ms |
| 100 `Perform`s, one file each | 7.3–7.5 s | 7.2–8.2 s | 17.2–17.4 s |
| One `Look` of 100 paths | 166–177 ms | 123–189 ms | 241–252 ms |

On the control machine, applying 100 files through the local executor took
1.45–2.2 s. Each change writes the state file durably: a `sync_all` on the file
and another on its directory. That is 202 `fsync`s for 100 changes, blocking
1.54 s in total, or 7.6 ms each, on ext4.

Through nerpa itself, 100 creates on Rocky took 13.95 s over a direct ssh and
14.06 s multiplexed, handshake included. Multiplexing is not a factor.

With larger files, over an open connection:

| | Rocky (Teleport) | Alpine (bastion) |
| --- | --- | --- |
| 100 × 64 KiB, one `Perform` each | 11.5 s | 28.6 s |
| 100 × 64 KiB, one `Perform` | 1.9 s | 2.4 s |
| 20 × 1 MiB, one `Perform` each | 4.2 s | 6.8 s |
| 20 × 1 MiB, one `Perform` | 2.7 s | 4.3 s |
| Reading the 20 MiB back, one `Look` | 1.2 s | 2.5 s |

The gain is for many small changes: 6–12× at 64 KiB, 1.5–1.6× at 1 MiB, where
transfer dominates.

### 2.2 Reading, for comparison

A converged plan across three machines with kept connections took 1.21 s at one
resource per machine and 1.45–1.55 s at a thousand. Cold, the same plans took
5.45–5.91 s and 6.52–6.88 s. A fingerprint (0011) could save at most the
difference, so this RFC does not pursue fingerprints.

### 2.3 What an executor does when its engine disappears

The batch was: create file a, `sleep 4`, create file b, `sleep 4`, create file c.
The local `ssh` was killed one second after the batch was sent.

| Route | The batch | The executor afterwards |
| --- | --- | --- |
| Teleport, as the user | completed: a, b, c | not reliably counted |
| Bastion, as the user | completed: a, b, c | not reliably counted |
| Teleport, `sudo -n` | completed: a, b, c | alive about 59 s after the client was gone, measured with a probe every second; the local proxy exited within a second of `ssh`, and SIGTERM changed nothing |
| Bastion, `doas -n` | completed: a, b, c | gone by +15 s |

No route stops a batch that has started. Stopping the local `ssh` gently changed
nothing on Teleport: the far side kept the session, and a root process, for up to
about a minute after its client was gone. An end of input cannot be relied on to
say "the engine is gone".
After a network partition it may not arrive for as long as TCP takes, because
server-side client liveness is off by default.

### 2.4 Replacing a running executor

These were run on Alpine, against the executor binary itself while it served a
conversation.

- Writing over it is refused with `Text file busy`.
- Renaming another file over it is allowed. The running process continues and
  ends normally when its input closes.

On Rocky SELinux is `Enforcing`. The installed executor is labelled `bin_t`, and
policy gives a new file in `/usr/local/sbin` `bin_t` too. A file moved in from
`/tmp` keeps its temporary label instead.

## 3. What is decided, and what is not

**Decided already.**

- **0031:** "One machine's batch is one blocking conversation." Fan-out is
  bounded and results are collected in a fixed order. It is built for reading
  only; `ARCHITECTURE.md` says "Applying does not yet". Its "state is written from
  one place" is corrected below.
- **0006:** stages order execution. Since `3d2d383` the planner's walk is taken
  again when holding one thing holds another, still before anything runs.
- **0015:** a second run with nothing changed has nothing to do, and a change to
  the plan's number for identical input shows up as a changed snapshot.
- **0026:** a triggered reaction is owed. Since `a8f55b6` the debt is written
  before its cause is sent (`INV-REACT-002`), and since `6e5bec8` on the record of a
  reacting resource the plan cannot reach, whose cause otherwise waits
  (`INV-REACT-006`).
- **0027:** a secret's value lives in memory for as long as a provider needs it.
- **0033:** the operation set is closed.
- **0034** and **0041:** a `Perform` carries a sealed premise, and the executor
  checks it before any operation.
- **0040:** a machine is spoken to only by the executor build this nerpa carries.
- **0042** and **0043:** a resource found as declared is recorded without being
  touched, and one whose edges changed is recorded again.
- **`INV-STATE-005`:** what a run has made is written down before it touches the
  next thing.
- **`INV-BATCH-001`:** one journey for reading.
- **`INV-REACH-002`:** only a machine not answering lets a run continue.

**Built since the first review, and relied on below.** A change carries every
edge it comes after (`85200af`). A change that sends nothing is marked so
(`ChangeKind::sends_anything`). A value too large for one operation is refused
while planning (`1856bc3`), and so are two resources naming one thing on a machine
(`7aed616`). A renamed resource that keeps its path is made again after its removal
(`842abcd`). Rousing restarts only a running service (`ae25cac`), and a provider is
told whether a change was roused (`4db7ffd`). A removal on a machine that did not
answer is not planned, nor is one waiting for it (`d7330a9`). A package cannot
react (`50021f0`). A declared resource naming what a held removal takes away waits
with it (`1e04f62`). A removal comes after the change of whatever state recorded
after it (`3d2d383`). A reaction on a machine that did not answer is owed on its
record (`6e5bec8`). A plan's number is written down (`27b2a2a`).

**Decided by the owner.**

- After the first review: what happens when something fails is **configurable**
  (§5.5).
- After the second: **state is kept in shards, one per machine** (§5.6); **a run
  that stops keeps the machines it was acting on out of the next run** (§5.8);
  **what reading cannot confirm is written down before it is sent,** and only that
  (§5.6).
- After the third: **this RFC keeps step 1 only**; **a write whose content is secret
  is answered with a sealed digest** (§5.3); **no absence premise** (§5.7);
  **packages cannot react** (built).
- After the narrow review: **a reacting resource the plan cannot reach is owed on
  its record, and one with no record holds its cause** (built); **revision 4 is
  reviewed again, for P and S, before acceptance.**

**Not decided, and what this RFC settles.**

- The provider contract for applying.
- How machines apply in parallel, and who writes state.
- What state knows when a run is interrupted.
- What a failure does to the rest of a run.
- How a person stops a run, and what the next run may touch.

## 4. Prior art

| System | Unit of work over the wire | Parallelism | Interruption | Keeping the agent current |
| --- | --- | --- | --- | --- |
| Ansible, `linear` | A task on a host; pipelining off by default | Up to `forks` (default 5) hosts per task, task by task | Per task | Not its business |
| pyinfra | An operation on a host; commands built at execute time | All hosts per operation, in lockstep | Per operation | — |
| Terraform | A resource's own API calls | `-parallelism`, default 10, over the graph | State keeps what completed before an error | — |
| Chef | The whole run, on the node | Per node | — | `chef_client_updater` kills the running client and its parent; `exec` over itself "highly discouraged" |
| Salt | The whole state run, on the minion | Per minion | — | Restarting the minion kills the running job; restart in the background with `bg: True` and `KillMode=process` |
| Teleport agent | — | — | — | The updater asks the proxy, rolls back immediately on failed health checks, updates in groups with a `canary_count` |

nerpa's batch is closest to the agents, which apply a machine's whole run at once.
The logic stays on the control machine (0032), and only a closed set of
operations crosses the wire (0033). Its executor is a process per conversation,
not a daemon, which removes the problem Chef and Salt have: nothing supervises it,
so nothing has to be killed to replace it.

## 5. Design

### 5.1 The unit

**A machine applies its changes one `Perform` at a time, and machines apply side
by side.**

- Machines are opened over a bounded pool, limited per route (§5.4).
- On a machine, changes go in plan order over the one conversation that machine
  has for the run, each once what it waits for has an outcome (§5.4, §5.5).
- A change that sends nothing, an adoption or a reorder, goes to the recorder once
  what it waits for has an outcome.
- A change whose resource is not on a machine runs on the engine, one at a time.
- A change its provider marks one at a time is sent and recorded before the
  machine's next change.
- **A replacement is two `Perform`s:** its delete, after which it is recorded as
  forgotten, and then its create, as today, so a failure never has to say which half
  ran. Every other change is one `Perform`.

### 5.2 Providers: `prepare` and `conclude`

```rust
/// The kinds of change that send something. Adoptions and reorders never reach
/// a provider.
#[non_exhaustive]
pub enum Sending { Create, Update, Replace, Delete, React }

/// Everything a provider is told to build one change.
pub struct Prepared<'a> {
    pub address: &'a Address,
    pub kind: Sending,
    /// Something it reacts to changed, or an earlier run left the reaction
    /// unpaid. Built: `Provider::apply` takes it since `4db7ffd`.
    pub roused: bool,
    /// Sent again: an earlier run marked it and never recorded it, and this run's
    /// reading found the resource (§5.6).
    pub resent: bool,
    /// Settled values, secrets included; empty for a removal.
    pub desired: &'a Attributes,
    /// What state recorded, for a replacement and a removal.
    pub recorded: Option<&'a Attributes>,
    pub facts: Facts<'a>,
}

/// What `conclude` and scheduling are told: the change with no secret value.
pub struct Concluding<'a> {
    pub address: &'a Address,
    pub kind: Sending,
    pub roused: bool,
    pub facts: Facts<'a>,
    /// Settled values that are not secrets, and the names of those that were.
    pub plain: &'a Attributes,
    pub secret: &'a BTreeSet<String>,
}

trait Provider: Send + Sync {
    fn read(&self, batch: &[Reading], facts: Facts<'_>, reach: &dyn Reach)
        -> Result<Read, ProviderError>;

    /// The operations that make this change. No machine.
    fn prepare(&self, change: &Prepared<'_>) -> Result<Planned, ProviderError>;

    /// What the change became, from exactly the answers to its own operations,
    /// once all of them have run. A file write that the engine checked arrives as
    /// `Seen::Written`.
    fn conclude(&self, change: &Concluding<'_>, became: &[Seen])
        -> Result<Concluded, ProviderError>;

    /// What the next run does with this change if a run marked it and never
    /// recorded it.
    fn afterwards(&self, address: &Address, kind: Sending) -> Afterwards;

    /// Whether this change is sent and recorded on its own.
    fn one_at_a_time(&self, change: &Concluding<'_>) -> bool { false }

    fn fake() -> Self where Self: Sized;
}

pub struct Planned {
    pub operations: Vec<Operation>,
    /// For a replacement, how many of the operations remove what was there. The
    /// change is recorded as forgotten after them. `None` for every other change.
    pub forget_after: Option<usize>,
}

pub enum Concluded { Became(Attributes), Gone }
pub enum Afterwards { Read, SendAgain }
```

The rules:

- **A provider never reaches a machine outside `read`.**
- **One change's operations are contiguous,** and `conclude` is handed exactly its
  own answers, in order, after the last of them.
- **A replacement is one `prepare`:** delete operations from `recorded`, create
  operations from `desired`, and `forget_after` between them. The engine sends them
  as two `Perform`s and records the resource as forgotten between the two.
- **A removal is given what was recorded and nothing desired,** and the provider
  decides what removal means for its kind: a file is removed; a service is stopped
  and disabled. A removal that only settles concludes `Gone` once settled, and the
  engine forgets the record on every `Gone` (C4-8).
- **Neither `conclude` nor scheduling sees a secret value.** A secret is fetched
  when its change is prepared, just before its first `Perform` is written, and lives
  inside the operation that carries it until that `Perform` is written (0027).
- **The engine checks a file write's answer,** not `conclude` (C4-1). When it seals
  a `Perform` it holds the value it is sending and the machine's seal. It computes
  there the digest the executor's answer must carry, and for a secret the digest
  state will record. It compares the answer itself: a match reaches `conclude` as
  `Seen::Written`, and a mismatch is a failure after the operations ran. So
  `INV-PLAN-005` reads: an attribute handed to a provider comes back from `conclude`,
  or is confirmed by a write digest the engine checked, and a secret's recorded
  digest comes from what was sent (C4-3).
- **A refusal from `conclude` follows operations that ran.**
- **A replacement interrupted between its halves stays forgotten,** and reading
  resolves the rest.
- **No branching inside a change.** A provider that needs an intermediate answer
  splits the work across stages (`W-009`).
- **`afterwards` is `Read` only where reading sees every declared attribute the
  change sets.** Files, directories and a service's settings are `Read`. A package
  is `SendAgain` for every kind of change that reaches it, its update included,
  because a package is registered before its post-install runs (C4-7). A resend is
  `dnf reinstall` on rhel, since installing a version already there runs no
  post-install, and `emerge --oneshot` of the same version on Portage, which merges
  it again. A change is marked resent only when this run's reading finds the
  package, so one not found is installed as usual (C4-2). A change's rousing is never
  `afterwards`' concern: its debt covers it (`INV-REACT-002`, `INV-REACT-006`).
- **Minted identity is not in step 1** (C4-5, C4-6, S4-14). No provider step 1
  ships has a resource whose identity a machine or a cloud mints, and this contract
  cannot express finding one again. A provider declaring such a kind is refused
  until an RFC designs a token in `Reading`, a `Read` that can say "found by the
  token", a token made before `prepare` and kept outside the plan's number, and
  `import` and `forget`.
- **A package cannot react,** refused when configured (`50021f0`).
- **`Sending` is `#[non_exhaustive]`,** and a provider refuses a kind it does not
  know rather than folding it into another arm.
- **Operations are idempotent, or observed.** Tested for files and directories with
  an in-memory executor; packages and services need a stateful fake, specified
  before their coverage is claimed (§5.11).
- **Every `ProviderError` is governed by the failure policy.** What always stops a
  run is internal to the engine (§5.5).
- **Facts are decoded twice per change,** once for each half. Accepted.

**Validating and preparing.** Every stage is validated before anything is sent:
each change's kind, size and edges, without building its operations or fetching
its secrets. A change is prepared, and its secrets fetched, just before its first
`Perform` is written. What the engine holds at once is then one change for each
machine it is sending to (§5.9), and a secret is never held for changes still
waiting (P4-15).

The trait changes now because only one provider implements it: the `os`
provider, plus the test fakes. That is the argument 0031 made for `Send + Sync`.

### 5.3 The wire: protocol 5

Protocol 4's shape stays: `Look`, `Perform` and `Build`, one answer each, premises
checked before a `Perform`'s operations, and the executor stopping at the first
premise that moved. What changes:

- **A file write is answered with a digest of the entry read back** after the write,
  its content, kind and mode included, never with the content. The digest is
  `Seen::Sealed`, sealed for the machine as a sealed observation is (0041), when the
  operation says the content is secret, and `Seen::Digest` otherwise (C4-4). The
  flag on a file write is required and has no default. A secret write in a `Perform`
  without a seal is refused before anything is looked at, which extends
  `INV-PREMISE-002`. The engine checks the answer (§5.2). So no answer carries a
  secret back, and no answer echoes a large file (§5.9). 0033's closed set gains the
  flag.
- **A private process is answered with its status and a digest of what it wrote to
  its output and its standard error, sealed for the machine,** and a failure names
  the status and never quotes what the process said. Today its output returns in
  the answer, and its standard error in a failure's problem despite "(input
  withheld)": measured, `cat` given a private input answered with that input (P4-2).
  No operation of the `os` provider is private yet; this closes it before one is.
- **A failure says where:** `Failed { at: Before | Premise(n) | Operation(n) }`, and
  `Observation(n)` for a `Look`. `Before` is a refusal before anything was looked at,
  such as a premise without its seal, and counts as a contract violation (P4-10).
- **A frame is decoded to its end, and unknown keys are refused.** Trailing bytes
  inside a frame are refused, where today they are ignored. The wire tests carry
  byte vectors.

`INV-WIRE-001` is unchanged: a frame of another version is refused by name. A
version 4 executor refuses a version 5 frame by writing to its standard error and
exiting without an answer (P4-9), so opening waits for that exit and reports the end
of the standard error beside the build nerpa carries. This is one reinstall on
every machine with an installed executor, and 0040 refuses a machine still carrying
the previous build until it is done.

**The conversation.** `Reach::ask` stays, and a conversation is used exclusively
from writing a request to reading its answer. While reading, failures keep today's
classes (`INV-REACH-002`); while applying, §5.5's table decides.

**Opening.** A handshake is retried only when `ssh` exited 255, no byte of an answer
was read, and nothing but `Build` was written, a few times with jittered backoff.
An exit of 1, or an answer naming another build, is never retried. Whether `ssh`
failed before authenticating cannot be observed: 255 covers every error of its own,
and its standard error can be empty (P4-11). That standard error is drained into a
buffer that keeps its last 64 KiB, for the report.

### 5.4 The engine: running a plan

1. **Validate every stage** (§5.2).
2. **Group by machine.** Adoptions and reorders go to the recorder. Control-domain
   and one-at-a-time changes are serialised on the engine.
3. **Wait per edge, not per stage.** A change begins once every change it waits for
   (§5.5) has an outcome durably written in its shard: recorded, failed, refused or
   unknown. This replaces a stage barrier, which held every machine's next stage
   behind the slowest machine in the fleet, and behind a request that never answers
   for as long as it did not answer (P4-7, B4-3, S4-12). Most changes wait only for
   changes on their own machine.
4. **Owe per cause.** Before a cause is sent, the debt of every reaction it rouses
   is durably written. A reactor on the cause's machine is owed in the same write
   that marks the cause (§5.6). A reactor on another machine, or one the plan cannot
   reach (`INV-REACT-006`), is owed in its own shard, and the cause waits for that
   write (B4-2). Owing per cause keeps 0026's window to the causes in flight rather
   than a stage's (S4-15).
5. **Open by route.** Connections are opened, and executors delivered, through a
   queue per route. A worker takes a machine to open only from a route with a free
   route slot, taking the global slot and the route slot together, and takes routes
   in turn among those with a free slot and a machine waiting (B4-4). A machine's
   route is what `ssh -G`, run with the fleet's configuration, says its first hop
   is: the first `proxyjump`, else the `proxycommand`, else the `hostname` (P4-8). A
   fleet may name routes itself, and then its names win. Applying reuses the
   connections reading opened and takes no route slot.
6. **Send** each change as its `Perform`s (§5.1). On each machine, a request is
   written only once the write recording the change before it, and marking this
   one, is durable (§5.6).
7. **The recorder owns what state says and in what order,** and has a writer per
   shard, writing in parallel, each with group commit. That corrects 0031's "state
   is written from one place". Measured on this control machine in one session, one
   small file per thread, written, synced, renamed and synced: 75 writes a second on
   one thread and 923 on sixteen, each the rate of one run (B4-7).
8. **On a failure,** the policy decides (§5.5).
9. **Stopping** is §5.8.

**What a machine waits for,** per change: the exchange itself, 73–174 ms per change
measured (§2.1); its shard's write, 13–20 ms up to 1000 resources on the machine
(medians of one session); and the write of another shard, only where an edge or a
debt crosses to it. A request that never answers holds the changes that wait for it
and nothing else. The report names its machine as in flight, and whether a request
should end on its own is `W-015`'s question (§8).

### 5.5 When something fails

Two policies, chosen per run.

**`stop`, the default.** The first failure anywhere sets the stop flag. No change
begins after it, by any path, and every failure is reported.

**`continue`.** A change that failed is refused, and so is everything that waits for
it, transitively. Everything else goes on. Every failure and every refusal is named
with its cause, and the run exits with a failure.

**What a change waits for,** carried in the plan and part of its number:

- **`depends_on`,** what state records a resource came after: a declared change
  after what it depends on (built, `85200af`).
- **`waits_for`,** shown and part of the number but never recorded, so that the
  next plan is never a reorder for an edge that only ordered this one (S4-2):
  - the removal of X after the removal of every doomed Y whose record names X;
  - the removal of X after the change of every declared Y whose record names X
    (built, `3d2d383`, `INV-PLAN-011`);
  - a declared change after the removal of what it names (`INV-PLAN-009`);
  - a replacement after the removals of doomed records that name it.
- **A cause's debts in other shards** (§5.4).

**Outcomes.** Each change ends in exactly one: recorded, failed, refused, stopped
at its forget point, not begun, or unknown (S4-18). A failure after a replacement's
forget point counts as failed. An adoption or a reorder counts as recorded once
written, and as not begun otherwise.

| Outcome | Counts as | The conversation | The change's mark |
| --- | --- | --- | --- |
| `Done`, a change's last `Perform` | recorded | kept | cleared by its record |
| `Done`, a replacement's first `Perform` | recorded as forgotten; its create follows | kept | kept, past the forget point |
| `Moved`, or `Failed` at a premise | failed; nothing of it ran | kept | withdrawn |
| `Failed` at an operation | failed after the operations before it ran | kept | kept as its entry for `SendAgain`; cleared for `Read` |
| A write digest that does not match, or `conclude` refuses | failed after its operations ran | kept | as above |
| `Failed` `Before`, an answer of the wrong kind or count, or an executor that exits on a request it cannot decode | a contract violation | ended | kept |
| The transport exits, or an answer does not decode | a conversation lost: that machine's remaining changes and what waits for them are refused | ended | kept; the machine is held (§5.8) |
| A failure to prepare | `stop`: nothing more begins; `continue`: that change and what waits for it are refused | — | none written |

Premises go on a change's first `Perform` only, so "nothing ran" is said only of a
failure there (S4-13). A lost conversation is reported as a machine lost part way,
never as one that did not answer to begin with (`INV-REACH-002`).

- **Always a stop, whatever the policy:** a state write fails, a contract is
  violated, or a signal arrives.
- **Reactions.** A reacting resource comes after its cause, so it is refused when
  its cause fails. Its debt was written before the cause was sent and is never
  withdrawn: at most one redundant restart for each cause in flight, the window 0026
  accepts.
- **Where it is set:** `[apply] on_failure = "stop"` or `"continue"` in the fleet,
  in a table of settings of its own whose unknown keys are refused, overridden by
  `nerpa apply --on-failure`, and printed in the plan's header.

**Invariants.** `INV-REACH-002` keeps its wording, is scoped to planning, and names a
held machine (§5.8). A new invariant, `INV-APPLY-001`: **no change begins, by any
path, after a change it waits for has failed or been refused; and under `stop`, none
begins after the stop flag is set** (S4-13). One refusal check in the recorder, which
every path consults, enforces it: a `Perform`, a control-domain change, an adoption
and a reorder alike. It is tested through a control-domain fake. The postcondition in
`apply` becomes: each change is in exactly one outcome, the outcomes add up to the
plan on return, and `Applied` carries each with its cause. A panic is covered by the
marks (§5.8).

**Risk, accepted:** `continue` leaves a fleet part changed on purpose. It is named in
every refusal, in the plan's header and in the exit status, and it is not the
default.

### 5.6 State: shards, marks, recording

**The directory,** `<state>.d/` (§5.13):

- `key`, the digest key, written and synced first; loading refuses shards without
  it.
- `format`, naming version 5, written last when the directory is made.
- A shard per machine, named by the hexadecimal of a SHA-256 of the target, so that
  a name has a fixed length whatever the target (S4-16), and `control` for the
  control domain. Each shard carries its target, and loading refuses a record whose
  target differs.
- `lock` (§5.8), and reports under names outside the hexadecimal alphabet, which
  loading ignores (P4-16).

A write rewrites only its shard. Leftover `.new` files are ignored. A shard that does
not parse refuses the run, because what it held — records, marks, debts — cannot be
told apart from nothing. Each shard is written whole or not at all
(`INV-STATE-004`, per shard). Two thousand shard files are listed in 0.4 ms, so
their number is not a cost here.

**What a write costs,** medians of one session on this control machine, one file:
13 ms at 100 resources, 20 ms at 1000, 60 ms at 10 000 and 538 ms at 100 000. Below
1000 the two `fsync`s dominate, and another session measured 1.3–2.6 times
differently. A shard holds one machine's resources, so its write costs that
machine's state rather than the fleet's.

**One write per `Perform`.** Before a machine's next `Perform` is written, its shard
durably holds, in one write:

- the record of the change before it, or of that change's forget point;
- the **mark** of the change about to be sent: its address, kind, locator, settled
  non-secret values, edges, the time, and whether it is past its forget point;
- the debts this change leaves on reactors in the same shard (§5.4).

The mark is cleared by the change's record, written with the next mark, or on its
own once the machine has nothing more to send. A change costs one durable write, not
two, and no file is shared between shards (B4-1, S4-6, P4-5).

**What a mark is for.**

- **Holding a machine.** A mark left by a run that ended holds its machine until a
  person releases it (§5.8). Marks name at least the machines the run was acting on:
  a mark can be written and its request never sent.
- **Writing ahead where reading cannot confirm.** A mark of a change whose
  `afterwards` is `SendAgain` is that change's pending entry.

An entry exists only for a change that was about to be sent, so a change never sent
leaves none, and nothing is resent that was never touched (P4-4). A failure at a
premise withdraws the mark in the write that records the failure.

**Bounded risk for what gets no entry.** A machine sends its next change only after
its previous one is recorded, so what state can miss is the change in flight on each
machine: up to one for each machine the pool is sending to, where today's serial loop
misses at most one. If that change made a file and its declaration is then removed,
the file is not removed; if it was a reorder, removal uses the old edges. Named in
`INV-STATE-005`'s bypass.

**How the next plan treats an entry.** One rule: an entry always yields a change that
sends something, never an adoption, a reorder or nothing (S4-4). Its kind comes from
this run's reading and the point passed: a replacement past its forget point is a
create. It is marked resent only when the reading finds the resource (C4-2). The
declaration counts as changed when the kind or the settled non-secret values differ;
edges do not count. The rule sits in the planner after the deltas and the reaction,
and before adoption, reorder and nothing. An address has at most one entry, and the
next mark replaces it.

| The entry | Its declaration | Planned |
| --- | --- | --- |
| `SendAgain` | still declared | the declaration's change, or the entry's kind where the declaration's would send nothing; resent if the reading finds the resource |
| `SendAgain` | gone | a removal built from the entry's locator, which joins the removals' order, holds and names (`INV-PLAN-009` to `INV-PLAN-011`) with the entry's edges |
| a removal | declared again | the declaration's change, sending something |
| a removal | still gone | the removal again, since already being gone is not a failure |
| any | on a machine that did not answer, or held | kept, and not planned |

"Resent" and the point passed are part of the plan's number, encoded for every
change, and a plan written without them is refused. The time a mark was written is
kept outside the number, as a premise is, and never shown as an age. Every plan's
number changes once for this, and the snapshot that 0015 asks for changes in the
same commit (S4-17).

**Clearing.** An entry is cleared only by the record of a change for its address that
sent something, by a removal's forget, or by a person.

**What a record holds for a file's content.** What the engine sent, once the answer's
digest confirms it: the value for plain content, as today, and the keyed digest for a
secret (S4-10). An adoption (0042) records what it read.

**Debts** live in the reactor's record (§5.4), including the record of a reactor the
plan could not reach (`INV-REACT-006`), and are never withdrawn.

**Group commit, per shard.** While one write of a shard is in progress, what arrives
for that machine queues, and the next write carries all of it. How soon an outcome
reaches disk is that next write, which its machine waits for.

**Invariants.**

- `INV-STATE-005` splits in three. "Nothing whose provider's `afterwards` is not
  `Read` is sent before a durable write marks it." "An entry is cleared only by the
  record of a change for its address that sent something, by a removal's forget, or
  by a person." "On each machine, a request is written only after the write recording
  the change before it is durable," which keeps today's adversarial test and gains a
  parallel one (S4-7).
- `INV-STATE-004` holds per shard; its lock clause becomes §5.8's.

**The properties** (S4-8).

- A completed run leaves the same state whatever order machines finish in, `written`
  aside.
- After an interrupted run, each change of the plan is in one of these cases: its
  completed record is written; it was not begun, and its address holds what it held
  before; it has a mark, as an entry or holding its machine; it is a replacement past
  its forget point, with no record; or its `afterwards` is `Read`, and reading at its
  recorded or declared locator resolves it. Records hold what was read, reported, or
  sent and confirmed. The tests generate removals, moved locators, replacements and
  changes not begun.
- **The run after the one that recovers is empty (0015),** except where a resend
  fails or a machine is still held.

### 5.7 Premise

**Checked before the operations of a change's first `Perform`,** as today, sealed
for the machine. A replacement's create is not checked against a premise its own
delete moved.

**No absence premise.** A create writes over what appeared between reading and
writing, as today. Revision 2 proposed a premise of absence, and the third review
found it would refuse what an earlier change of the same run created, a package's
default configuration file among them (P3-12). **Risk, accepted:** a file put where
a create is about to write, by somebody else during the run, is written over.

**P-6** was not reproduced and is narrower than first stated: it needs a second
package planned as an update whose version the first one's install changes.
Per-change premises refuse that update, and the next run converges.

### 5.8 Stopping, the lock, and held machines

**`lock`** is a file in the directory, created once and never removed, and held with
`flock` while a run is alive, so the kernel releases it when the process ends,
however it ends. Its descriptor is close-on-exec, which is Rust's default, and that
is a requirement: a transport that inherited it would hold the lock for as long as it
outlived nerpa, about a minute on Teleport (§2.3). Every command that writes the
directory takes it: `apply`, `release`, the migration and the downgrade (P4-13,
S4-19). It keeps out another live run and nothing else.

**Held machines.** A machine whose shard holds a mark from a run that ended is held.
The next run reads nothing from it and plans nothing on it, and says so: the plan
carries `Held { node, since, change }` beside `Unreachable`, counted by
`is_complete` and by the number, and a hold spreads along edges exactly as
unreachability does, removals and reactions included (S4-11). A held machine is not
reported as one that did not answer, and `INV-REACH-002` names the hold.

**Releasing.** In this RFC only a person releases a machine, with
`nerpa release <machine>`, because protocol 5 cannot show that nothing of the old run
is still running there; RFC 0003 designs what could. `release` takes `lock`, and
refuses beside a live run. It clears the machine's marks and nothing else: the mark
of a `SendAgain` change stays as its pending entry, and the mark of a `Read` change is
dropped, so the next run reads it. Records and debts stay. The tests: a released
machine keeps its pending entry, and `release` refuses while a run holds the lock.

**Stops end at change boundaries** (P4-3). A signal, the first failure under `stop`,
and a panic each set the stop flag, and no worker then begins a change. A change past
its forget point sends its create within the wait unless its own answer failed, so a
stop never leaves a path removed and not made again on a machine that did not fail.
One that cannot finish within the wait is reported as stopped at its forget point.

**Signals.** SIGINT, SIGTERM and SIGHUP stop the run. The first stops new changes and
waits a bounded time for the answers in flight, recording them as they come. A second
signal kills the transports, except that a SIGHUP and a SIGTERM close together, as a
closing terminal sends them, count as one; a second SIGINT always counts (P4-16).
Signals are taken through a self-pipe, never by blocking or ignoring them in the
engine, because a blocked or ignored signal is inherited by every child: `ssh`, the
local executor, and what it runs. Measured: a program the executor ran showed the
engine's blocked and ignored signals in `SigBlk` and `SigIgn` (P4-14). Children start
with an empty mask and default dispositions, reset in the `pre_exec` that calls
`setsid`. SIGTSTP is caught while applying and does nothing. SIGTTOU and SIGTTIN keep
their defaults: a backgrounded `apply` that touches the terminal is stopped as any
program is, and keeps its marks. A run stopped with SIGSTOP keeps its marks, and reads
the answers when it continues.

**Ending a wait** (P4-6). Each transport's kill handle is kept outside the
conversation's mutex, so a stop can end a blocked read. Every wait, for a record, a
debt or an answer, also watches the stop flag. When the first signal's wait ends, a
supervisor flushes the recorder, writes the reports and kills the transports, and a
thread left in a read ends with its pipe. A panic in the recorder ends the process
once the reports are written, since nothing else can record. The marks name the
machines.

**A panic stops the run.** A panic hook sets the stop flag, and nothing on unwind
removes a mark. Reports are written with their errors ignored, and after SIGHUP they
are written into the directory as well, because a failed write to a closed terminal
panics.

**Transports run in a session of their own** (`setsid`), so the terminal's signals
reach nerpa and not `ssh`, and a prompt that reads the terminal fails at once. An
askpass program, or a browser opened by a proxy command, is not covered. Closing
waits a bounded time, then kills.

### 5.9 Size

**A change's encoded size is bounded while planning,** the total of its operations,
at `MAX_CONTENT` (32 MiB), where `INV-PLAN-007` bounds one attribute today. A change
too large is refused before anything is sent. The constant was half a frame because a
file's answer held its content again; protocol 5 answers a write with a digest, and
the bound is kept so that planning refuses what one request could not carry (B4-8).

**Memory while sending:** one change in flight for each machine the pool sends to,
prepared just before its `Perform` (§5.2). Sealing and writing a request copies it:
the ask, the sealed ask and the encoded buffer are three copies today, so up to about
96 MiB for each machine, and about 1.5 GiB at sixteen, until sealing borrows the ask
and consumes it (B4-5). The pool size is the lever.

**Memory while reading** is not bounded by this RFC. A `Look` is answered with a
file's content, up to `MAX_CONTENT` per file, for a machine's whole batch at once.
Stated, not changed.

### 5.10 Limits the transports impose

| Limit | Value | Consequence | Response |
| --- | --- | --- | --- |
| OpenSSH `MaxStartups` on a shared bastion | `10:30:100` by default | More than 10 unauthenticated connections start being dropped at random | A slot per route while opening and delivering, default 8, inside the global 16, both in the fleet's `[apply]` table; the route from `ssh -G` (§5.4); retry as §5.3; recommend `ControlMaster` on the jump hop |
| OpenSSH `MaxSessions` | 10 per connection | Forwarding channels are not named | Verify before relying on multiplexing through a jump host (§8) |
| Teleport role `max_sessions`, proxy `connection_limits` | 10 per connection; 1000 per client address | nerpa opens one session per connection | From Teleport's documentation, not measured on this proxy |
| One `ssh` process per machine | `W-014` | Memory and process count on the control machine | Bounded by the pool; a conversation holds no thread of its own in this RFC |
| Durable state writes | 2 `fsync` per write; a write costs its shard | One write per `Perform` | A writer per shard with group commit (§5.4, §5.6) |
| Memory | about three copies of a change per machine in flight | about 1.5 GiB at sixteen machines, at the largest change | The pool size (§5.9) |
| A request that never answers | no deadline | Holds what waits for it | Named in the report; `W-015` (§8) |
| The transport's standard error | a bounded buffer | A verbose route cannot stall `ssh` | The last 64 KiB kept |

### 5.11 Testing

- **An in-memory executor that interprets operations:** replay tests for files and
  directories, and a stateful fake registry per package family and init system,
  specified before package and service coverage is claimed.
- **The recorder property,** per change (§5.6), for completed and interrupted runs,
  generating removals, moved locators, replacements and changes not begun.
- **The failure-policy property** over every edge in §5.5, `waits_for` and debts in
  other shards included, and through a control-domain fake.
- **Interrupted, then two runs:** files, directories and the package fake converge,
  and the second run is empty.
- **Stopping:** a panic in a worker, in the recorder and in a reader, in turn, leaves
  marks naming at least the machines with a request unanswered; a killed run leaves
  the same; the next run holds them until released. `stop` on one machine while
  another is between the halves of a replacement leaves the replacement made.
- **A far end that never answers:** a signal ends the wait, and only what waits for
  that machine is held back meanwhile.
- **Migration:** a crash at each step from format 4 to 5 leaves a state that loads, or
  a refusal, and never an empty first run; a version 4 binary run afterwards refuses.
- **Scheduling:** three routes with more machines than slots; routes are served in
  turn, and a saturated route never holds a worker another could use.
- **Protocol 5:** byte vectors for every new shape; a secret write is answered with a
  sealed digest and never its content; one without a seal is refused `Before`; a
  private process is answered with sealed digests; unknown keys and trailing bytes
  are refused.
- **A gate like `xtask budget`** for writes a second per shard under concurrent
  outcomes, off the commit path, fixed as `docs/budgets.toml` fixes its own: 50
  machines, 200 changes each of a 1 KiB file, 1000 resources per shard, three samples,
  a release build, with the baseline and budget recorded from the first run (B4-6).
- **Step 1 measured on its own,** on the same simulated fleet: wall time and state
  writes a second, today's serial loop against this.

### 5.12 Failure analysis

| Event | Machine | Engine knows | State says | This run, then the next |
| --- | --- | --- | --- | --- |
| A premise moved | Nothing of that change done | `Moved` | Its mark withdrawn | `stop`: the run stops. `continue`: it and what waits for it are refused. The next run plans it again |
| An operation fails | Operations before it done | `Failed` at an operation | Recorded, or forgotten past a replacement's forget point; a `SendAgain` entry kept | The policy decides; the next run reads, or resends what reading cannot confirm |
| A write digest does not match, or `conclude` refuses | Its operations ran | A failure after action | As above | As above |
| An answer of the wrong kind or count, `Failed` `Before`, or an executor that exits on a request it cannot decode | That request's outcome unknown | A contract violation, with the executor's standard error | Its mark kept | Always a stop; the machine held until released |
| A signal, once | A change past its forget point finishes; answers in flight come, or not within the wait | Their answers, as they come | Recorded; the marks of the unanswered kept | Those machines held until released |
| A second signal | Transports killed. A remote executor finishes its current request; the local one (`Route::Here`) is killed with its transport, and a file it was writing can be left short | What was in flight, printed | Marks name them | As above; the next read finds the short file and writes it again |
| The engine killed, or out of memory | Remote executors finish their current request | Nothing more | Each shard's last write; marks name the machines | As above; `lock` released by the kernel |
| A panic in a worker or a reader | As a signal | As a signal | As a signal | As above |
| A panic in the recorder | As a signal | Reports written, then the process ends | Each shard's last write | As above |
| A request that never answers | Unknown | Waiting | Its mark | What waits for it waits; a signal ends the wait; the machine held |
| A conversation lost | The request's outcome unknown | Lost part way | Its mark | That machine held until released |
| The machine reboots mid-change | Same, and a write the executor never synced may be lost | Same | May record a write the machine lost | Once released, the next read sees the difference |
| A state write fails | Nothing more sent to any machine | A failure to write state | Each shard's last successful write | Always a stop; a person fixes the disk |
| A machine carries a version 4 executor | Refused at opening (0040) | The executor exited without answering; the end of its standard error, beside the build nerpa carries | Unchanged | Left out until its executor is reinstalled |
| Two engines on one machine | Two conversations | Each knows its own | Each lock protects its own state | Premises catch overlapping updates; no lock across engines (§8) |

### 5.13 Migration

**Format 5 lives at a path of its own,** `<state>.d/`, never in the version 4 file's
place (S4-9). A directory cannot be renamed onto a file, and renaming in the other
order leaves a moment with no state, which today loads as a fresh key and an empty
state: an empty first run.

**Loading.** A directory with `format` wins, and a version 4 file beside it is refused
by name. A version 4 file alone is migrated. A directory without `format` is a
migration that did not finish: it is removed and the migration runs again.

**Migrating.** Take the version 4 lock, by exclusive create as version 4 does, and the
new `lock`. Build the directory: `key` carried over from the version 4 file and
checked by recomputing a recorded digest, then every shard, then `format`, each synced,
and the directory synced. Then rename the version 4 file to `<state>.v4`, and leave at
`<state>` a file that a version 4 binary refuses by its version, so that it never
starts empty (P4-13). `<state>.v4` holds the live key; it may be removed once no
downgrade is wanted (S4-19).

**One-way, except through the downgrade command,** which takes both locks and refuses
while any shard holds a mark.

| Changes | Protocol | Reinstall | Invariants |
| --- | --- | --- | --- |
| `prepare`, `conclude` and `afterwards` (`Read` and `SendAgain`); machines in parallel with a `Perform` per change, two for a replacement; waits per edge; the recorder with a writer per shard and one write per `Perform`; marks as entries and holds; state format 5 at `<state>.d/`; the failure policy; `lock`, `Held` and `release`; signals through a self-pipe, and `setsid`; queues per route for opening; a bound on a change's size; a handshake retried on `ssh`'s exit 255 before any answer | 5: write answers as digests of the entry read back, sealed for secret content; private processes answered with sealed digests; failures name `Before`, a premise, an operation or an observation; frames decoded to the end, unknown keys refused | Once, on every machine with an installed executor | `INV-STATE-005` split in three; `INV-STATE-004` per shard; `INV-BATCH-001` extended to name applying; `INV-REACH-002` scoped to planning and naming holds; `INV-PLAN-005` for write digests; `INV-PREMISE-002` for secret writes without a seal; `INV-APPLY-001` new |

## 6. Alternatives

- **Batches now,** which was step 3. Not chosen: they wait for this and for what this
  alone gains (RFC 0003).
- **`conclude` seeing secrets, reduced in the recorder.** Not chosen, by the owner:
  the value would cross threads and provider code, where a sealed answer keeps it on
  the machine.
- **`conclude` checking a write's digest.** Not possible: it holds neither the value
  nor the seal (C4-1).
- **An absence premise on creates.** Not chosen, by the owner: it refuses what the
  same run created (P3-12).
- **A package reacting by being reinstalled.** Not chosen, by the owner: a stray
  reaction would reinstall a package.
- **A cause refused whenever a reacting resource cannot be planned.** Not chosen, by
  the owner: a change on a machine that answered would wait even where a debt can be
  written.
- **Recovery by reading alone.** Chosen for what reading can confirm; not for
  packages.
- **Minted identity in step 1.** Not chosen: no provider step 1 ships needs it, and
  this contract cannot express finding one again (S4-14).
- **Confirming a pending entry by reading, or treating it as unrecorded.** Not
  chosen: both still need a rule per kind, one adds a kind of change, and the other
  calls a resource nerpa already owned adopted.
- **State in one file with a stated limit, or a log with compaction.** Not chosen:
  the first stops helping at the fleet 0031 aims at, and the second is harder to
  read by hand and to recover from a torn tail.
- **`in-flight` as a file of its own.** Not chosen: shared by every shard's writer, it
  loses marks or puts a serial write before every request (B4-1).
- **A stage barrier.** Not chosen: one machine held every machine's next stage, for as
  long as its request did not answer (P4-7).
- **One lock file for both jobs, released after waiting, or kept whole until a
  person unlocks.** Not chosen: a killed run never rewrites it, and one stuck
  machine would stop a fleet.
- **One failure policy for everyone.** Not chosen, by the owner's decision.

## 7. What this costs

- **The provider trait changes,** and every provider says per kind whether reading
  confirms a change. Paid now, while one provider exists.
- **The engine gains a recorder, a writer per shard, waits per edge, a scheduler per
  route, signal handling through a self-pipe, a supervisor and a panic hook.**
- **State becomes a directory,** format 5, at a path of its own, and the migration is
  one-way except through the downgrade command.
- **Protocol 5 is one reinstall** on every machine with an installed executor, and
  0033's closed set gains a required flag on a file write.
- **Every plan's number changes once,** for `resent` and the point passed.
- **A package interrupted mid-install is installed again,** whether or not that
  install finished.
- **A run that stops leaves machines a person must release.**
- **A request that never answers holds what waits for it** until a signal.
- **A change waits for a reacting resource with no record** on a machine that did not
  answer (`INV-REACT-006`).
- **Sending holds about three copies of a change per machine** until sealing borrows.
- **A failure policy is a setting,** and one of its values leaves a fleet part
  changed on purpose.
- **A file appearing where a create writes during a run is written over.**

## 8. Open questions

1. **The bounded waits:** for answers in flight after a signal, and for closing a
   transport.
2. **Multiplexing through a jump host.** Is `MaxSessions` counted for forwarding
   channels?
3. **Two engines against one machine.** Is a lock across engines worth having, and
   where would it live?
4. **When the executor syncs what it writes** (B-6).
5. **A failure policy per group.** Not proposed.
6. **A journal of each run beside state,** for a person, outside any plan.
7. **Releasing a machine without a person,** which needs what RFC 0003 designs.
8. **A budget per request** (`W-015`): whether a request that never answers should end
   on its own, and what its outcome would count as.

## 9. Review log

Five reviewers, one angle each, on 2026-09-11:

- **P** protocol and failures, **S** state and invariants, **X** security of
  self-replacement: Opus.
- **B** performance, **C** the provider contract: Sonnet.

Every finding was checked before it was answered, in one of three ways:

- **E**: by experiment on the fleet or locally;
- **R**: by reading the code;
- **D**: accepted as reasoning about the design.

Severities are the reviewers' own, except where a verdict below says otherwise.

### Defects in the code as it stands, found by this review

These are true today, whatever becomes of this RFC.

| From | How verified | Defect | Fixed |
| --- | --- | --- | --- |
| S-2 | E | A resource that already matches its declaration is never recorded. Plan and apply say "nothing to do", no state file is written, and when the declaration is removed later nothing is planned: the file stays. | `6f70c8a`: adopted and recorded, [0042](../decisions/0042-a-resource-found-as-declared-is-adopted.md) |
| S-2 | R | State records only references still unresolved at plan time (`run.rs:411-421`). `after` edges and resolved references are lost, so removal order taken from state is incomplete. | `85200af`: every edge, carried by the change |
| S-3 | E | Two addresses with one path are accepted. Both are created, the last write wins, and every later plan proposes the other again. | `7aed616`: refused while planning |
| S-4 | R | A reaction's debt is written only when the reacting resource's own stage runs (`run.rs:348`). A run stopped between the cause's stage and that one leaves the new file recorded and the reaction lost. | `a8f55b6`: owed before the cause is sent |
| B-4 | R | Nothing checks an operation's size before sending. A file over `MAX_FRAME` plans cleanly and fails at apply. | `1856bc3`: refused while planning |
| P-6 | R | A package's premise is its query output. Installing A pulls in B and changes B's premise, so a B declared separately in the same stage is refused with `Moved`. Not reproduced on a machine. | Not reproduced, and narrower than stated: see the second review |
| P-12 | R | A conversation is not dropped after a wire error; only `Drop` clears it. A later ask would read out of step. Latent: nothing asks again today. | `74db388` |
| X-3 | R | The fleet accepts a relative or `~` executor path. `plainly` checks characters, not that the path is absolute. With `become`, that is the executor 0039 exists to refuse. | `e2ca0f8` |
| X-8 | R | The number of answers is not checked against the number of operations (`provider-os/src/lib.rs:86-88`). | `37ae618` |
| X-10 | D | 0039 says the sudoers rule now "means what it says". An executor running as root runs any process (0033), so the rule is root for that account. 0039 needs a Corrected line. | `bbbf891`: a Corrected line on 0039 |
| B-6 | R | The executor never syncs what it writes. A machine that loses power can lose a write the engine has recorded; the next read heals it. | Deferred to the executor work, which reinstalls it anyway |

Found while fixing them, and fixed the same way: a failing test first, then a
commit of its own.

| Defect | Fixed |
| --- | --- |
| A resource that already matched and was recorded kept the edges it was made with. An `after` written in later never reached state, so a file and its directory were still removed together in any order, and state from before `85200af` stayed short of edges. | `e1f0d1a`: planned as a reorder, [0043](../decisions/0043-what-a-resource-comes-after-is-recorded-when-that-changes.md) |
| Adoption and reorder counted as changing, so a service reacting to an adopted file was planned to restart on every machine a first run adopted. Never released. | `715858c` |

Two resolutions were built differently from what the tables below say. S-2:
adoption is recovery again, because a written file whose record never came is
adopted by the next plan. S-4: `owe` was not made to work without a record; a
reacting resource that state does not record yet is recorded from what was
read, so the state format did not change.

### P: protocol and failures

| Id | Severity | How verified | Finding | Resolution |
| --- | --- | --- | --- | --- |
| P-1 | blocker | R: no `process_group` or `setsid` anywhere | Ctrl-C reaches `ssh` too, so no pipe is left for `Stop`; `Piped::drop` waits without bound | Spawn transports in their own process group; a bounded wait, then kill |
| P-2 | blocker | R | `Reach::ask` returns one answer under one mutex, so `Stop` and `Alive` cannot be sent while a stream is read | A streaming conversation with separate send and receive halves; `Alive` independent of the recorder |
| P-3 | major | D | Frames carry no request id, so a stray `Working` or `Stopped` after a final frame shifts every later answer | Sequence numbers; exactly one final frame per `Perform` |
| P-4 | major | D | "At most the operation in flight is unknown" is false: Teleport lingering, queued commits | Superseded by S-1 |
| P-5 | major | R | A replacement is two recordable events | Record points inside a change, with S-5 |
| P-6 | major | R | Chunks can split a change, and a premise checked per chunk makes the outcome depend on where chunks fall | Cut only between changes; check a batch's premise once, before the first chunk; also a defect today |
| P-7 | major | D | `Alive` only during `Perform` leaves idle machines between stages, stale timestamps during large writes, and stops nobody asked for | `Alive` for the whole conversation; `Stopped` carries a reason; liveness timed from the last byte |
| P-8 | major | D | `Working` proves the process is alive, not that the operation moves; `W-015` is not narrowed | Say so; a per-operation budget is a separate decision |
| P-9 | major | E | The 13–69 s figure was two probes, and SIGKILL against SIGTERM did not separate the route from the proxy | Measured again, §2.3: the route |
| P-10 | major | R | A version 4 executor exits on a version 5 frame, and there is no version 4 decoder | A frozen bootstrap exchange for `Build` and replacement, with X-9 |
| P-11 | minor | R | `Failed.at` indexes the premise and the operations alike | A failure says which |
| P-12 | minor | R | A defect today, above | Drop the conversation on any wire error |
| P-13 | minor | D | Step 1 is capped by serial fsync, about 66 changes a second; it cannot stop other machines, and `record` is not callable from threads | The recorder, a shared stop flag and group commit move into step 1 |
| P-14 | nit | D | Retrying a handshake can repeat work | Retry only before the first `Perform`; capture stderr and exit status |

### S: state and invariants

| Id | Severity | How verified | Finding | Resolution |
| --- | --- | --- | --- | --- |
| S-1 | blocker | D | The reworded `INV-STATE-005` describes speed, not order; queued completions are lost | Write intent ahead. Pending records are written durably before a batch is sent, marked done on `Progress`, and a pending record is resolved by the next read. "What a run is about to make is written down before it is sent." |
| S-2 | blocker | E, R | Adoption does not record, and edges are lost | Defects today, above; stop citing adoption as recovery. Fixed, and adoption is recovery again |
| S-3 | major | E | Stage independence holds in the graph, not on the machine; creates and removals carry no premise | Refuse duplicate identity when the graph is built; re-check each change's own premise before its operations; never split a change |
| S-4 | major | R | The debt is written too late | Before sending the batch that holds a cause, durably owe every reaction it will rouse; `owe` works without a record. Fixed, recording the reacting resource instead |
| S-5 | major | R | The delete half of a replacement is recorded on its own today | Record points declared in `prepare`, with P-5 |
| S-6 | major | R | Step 1 does not keep `INV-STATE-005` without a recorder and a stop flag | With P-13 |
| S-7 | major | D | "The first failure stops everything" contradicts `INV-REACH-002` and 0040 | **Decision needed:** continue, refusing dependents one resource at a time (which puts edges into the plan), or scope `INV-REACH-002` to plan time |
| S-8 | major | R | Control-domain changes inside a stage, and an immediate exit that leaves the lock | Never time out control-domain or one-at-a-time changes; the second Ctrl-C shows what is in flight; the lock is handled explicitly |
| S-9 | major | D | Replacing an executor inside a plan conflicts with a machine left out when it was opened | Replacement as a separate step before reading, then a fresh plan for that machine, with X-5 |
| S-10 | minor | D | Records carry `Written::now()` | The property holds for completed runs ignoring `written`; a subset property for interrupted runs |
| S-11 | nit | D | Wording; mapping `Moved` back to a change; `Perform.plan` is always `None` | Taken |

### X: security of self-replacement

| Id | Severity | How verified | Finding | Resolution |
| --- | --- | --- | --- | --- |
| X-1 | blocker | R | The engine would install whatever `nerpa-executor.static` beside it holds, downgrades included; a hostile executor self-reports `Build` | Verify on the engine: a release manifest (digest, architecture, protocol, a build number that only rises), built into nerpa or signed; refuse downgrades; replacement is opt-in |
| X-2 | major | D | A check on the machine protects nothing a compromised engine cannot route around; the first key arrives unsigned; no revocation | Say so; sign a manifest and verify on the engine |
| X-3 | major | R | Relative paths; a symlink guard that checks then acts; no owner checks; `ln` and `mv` into symlinked directories | An absolute, normalised path (a defect today); root-owned parents nobody else can write; a unique `.new` with `create_new`; `ln -nf --` and `mv -Tf --` |
| X-4 | major | D | Two engines share the `.new` name; no premise on `P`; no sync before the rename | Unique names; a premise on `P`; sync before the rename; keep the old conversation to roll back through |
| X-5 | major | R | Opening refuses before planning, so replacement would talk as root to an unrecognised build, happen during `plan`, and apply resources never read | A replacement-only mode for an unrecognised executor, under `apply` only, as a planned change; that machine's own changes wait for the next run |
| X-6 | minor | D | The health check proves little, drops confinement and has no timeout | Compare labels, add a timeout, roll back automatically, and state what it proves |
| X-7 | minor | D | `Stop` between operations can split a change | Stop only at change boundaries |
| X-8 | minor | R | The stream is not validated; the answer count is unchecked today | Validate order, range and finality; a deadline per stage |
| X-9 | minor | R | Detecting an old version looks the same as a dropped connection; decoding one as a subset | A frozen bootstrap exchange; separate frozen version 4 types |
| X-10 | nit | D | 0039 overstates the sudoers rule; `P.previous` and globs; leftovers; `ControlPersist` lifetime | Correct 0039; exact rules only; clean up `.new`; name where the socket lives and for how long |

### B: performance

| Id | Severity | How verified | Finding | Resolution |
| --- | --- | --- | --- | --- |
| B-1 | blocker, judged major | E, R | Every state write serialises everything | Measured 14.8 ms a change at 1000 resources (1.35 MB) against 16.0 at 100; removals 13.9 to 19.9 ms. A limit of scale, not a blocker here. §5.6 will say a commit costs the size of state; a W-entry for the state format at 10k resources and more |
| B-2 | blocker, judged major | E | Only 1 KiB was measured | Measured, §2.1: the gain is for many small changes |
| B-3 | major | R | Several extra copies of every buffer | Borrow when sealing; a consuming `into_body`; a streamed `digest` |
| B-4 | major | R | No ceiling on the size of one operation | A defect today; refuse at plan time |
| B-5 | major | R | `AT_ONCE` is global and fixed | A global cap with per-route caps inside it, configurable; a simulated-fleet benchmark |
| B-6 | minor | R | The executor never syncs | A defect today; the sync policy is an open question |
| B-7 | minor | R, E | Read-back reads whole files: 1.2–2.5 s for 20 MiB | For large files, a digest instead of the content, in step 3 |
| B-8 | minor | D | The recorder can back-pressure every machine | Machine threads never block on the recorder |
| B-9 | nit | D | The headline figure is for batchable changes | Taken |

### C: the provider contract

| Id | Severity | How verified | Finding | Resolution |
| --- | --- | --- | --- | --- |
| C-1 | blocker | R: `uninstall` checks "still installed" | `prepare_removal` has no conclusion, so removal loses its check that the thing is gone | One `prepare` and `conclude` for every kind, deletion included |
| C-2 | major | D | `Prepared` is undefined, and `conclude` needs facts | `Prepared { address, kind, attributes, facts }`, passed to both |
| C-3 | major | D | `one_at_a_time` cannot see the kind of change | It takes `&Prepared` |
| C-4 | blocker | D | No order between one-at-a-time changes and the batch on one machine (a group before its user) | Within a machine and a stage, one-at-a-time changes complete and are recorded before the batch is sent |
| C-5 | major | D | One failed `prepare` blocks its siblings | Changed from the reviewer's proposal: prepare every change of a stage before sending anything, and if any fails, send nothing in that stage and report them all, as a refusal before action |
| C-6 | minor | D | Idempotency is not enforced | A shared in-memory executor that interprets operations, and a replay test per provider |
| C-7 | nit | D | Facts are decoded in three places | A helper |

### Revision 1, 2026-09-11

What changed after the first review, finding by finding. "Fixed" points to the
defects above; everything else points to where the design answers it.

| Findings | Answered in | How |
| --- | --- | --- |
| P-1 | §5.5 | Transports in their own process group; a bounded wait, then kill |
| P-2 | §5.5 | A conversation with separate send and receive halves replaces `Reach::ask`; `Alive` comes from the writer |
| P-3, X-8 | §5.3 | Sequence numbers, one final frame per `Perform`, and a validated stream; answers are counted today (`37ae618`) |
| P-4 | §5.7 | Superseded by writing ahead |
| P-5, S-5 | §5.2, §5.7 | Record points declared by `prepare` |
| P-6 | §5.8, §8 | The premise is checked once per batch, with the reasoning; the defect in step 1 is reproduced before anything is decided |
| P-7 | §5.3, §5.4 | `Alive` for the whole conversation; `Stopped` carries a reason; liveness timed from the last byte |
| P-8 | §5.4 | Said; `W-015` is not narrowed |
| P-9 | §2.3 | Measured again |
| P-10, X-9 | §5.3 | A frozen bootstrap for `Build` and replacement, carried by the next executor installed however it gets there |
| P-11 | §5.3 | `Moved` names a premise and `Failed` an operation |
| P-12 | Defects | Fixed |
| P-13, S-6 | §5.7, §5.14 | The recorder, stop flag and group commit move into step 1 |
| P-14 | §5.3 | Retry only before the first `Perform`; exit status and standard error kept |
| S-1 | §5.7 | Pending entries written before a batch is sent and resolved by the next read; `INV-STATE-005` reworded; recovery by reading alone is logged as the alternative (§6) |
| S-2 | Defects | Fixed; adoption is recovery again |
| S-3 | §5.1, §5.8 | Duplicate identity refused (`7aed616`); a change is never split; the premise is checked once per batch rather than per change, with the reasoning |
| S-4 | Defects | Fixed |
| S-7 | §5.6 | Owner's decision: `stop` by default, `continue` on request, dependents found through the edges the plan carries |
| S-8 | §5.1, §5.5 | Control-domain changes are never timed out; the second Ctrl-C prints what is in flight and removes the lock |
| S-9, X-5 | §5.10 | Replacement is a plan of its own before reading; that machine's changes wait for the next run |
| S-10 | §5.7, §5.12 | The property for completed runs and for interrupted ones |
| S-11 | Throughout | Taken |
| X-1, X-2 | §5.10 | Owner's decision: a manifest verified on the engine, opt-in per machine, never a downgrade; signing later |
| X-3 | §5.10 | The fleet's path is fixed (`e2ca0f8`); parents checked, an exclusive `.new`, `ln -nf --` and `mv -Tf --` |
| X-4 | §5.10 | A unique name, a premise on `P`, a sync before the rename, and restoring through the old conversation |
| X-6 | §5.10 | A timeout, labels compared, restoring, and what the check proves |
| X-7 | §5.1, §5.3 | Stopping only at change boundaries |
| X-10 | Defects | Fixed |
| B-1 | §5.7 | The cost of a write is stated; a watchlist entry for the format at scale |
| B-2, B-9 | §2.1 | Measured; taken |
| B-3 | §5.5 | Borrowed when sealing, a consumed answer body, a streamed digest |
| B-4 | Defects | Fixed |
| B-5 | §5.11 | A limit per first hop inside the global one, configurable, with a simulated fleet |
| B-6 | §5.13, §8 | The reboot row says what is lost; when to sync is open |
| B-7 | §5.14 | A digest rather than the content when reading back a large file, in step 3 |
| B-8 | §5.5 | A machine's threads never block on the recorder |
| C-1, C-2, C-3 | §5.2 | `prepare` and `conclude` for every kind, removal included; `Prepared`; `one_at_a_time` takes it |
| C-4 | §5.1 | One-at-a-time changes before the machine's batch |
| C-5 | §5.2, §5.6 | A stage is prepared whole first; the policy decides what a failure to prepare does |
| C-6 | §5.12 | An in-memory executor and a replay test per provider |
| C-7 | §5.2 | One helper |

Also new since the review, and folded in: an adoption or a reorder sends nothing
and goes straight to the recorder (§5.1), and a debt is written before its cause
is sent (§5.5). A second review is pending.

## Second review, 2026-09-11

The same five angles and models, on revision 1 (`a59c0b0`). Every finding was
checked before it was logged, in the same three ways.

**The first round, as revision 1 left it.**

- **P:** P-1, P-2, P-3, P-4 (superseded), P-8, P-12 and P-14 resolved; P-5, P-6,
  P-7, P-9, P-10, P-11 and P-13 partly.
- **S:** S-2, S-4, S-6 and S-9 resolved; S-1, S-3, S-5, S-7, S-8, S-10 and S-11
  partly.
- **X:** X-2 and X-8 resolved; X-1, X-5 and X-7 resolved in the design only; X-3,
  X-4, X-6, X-9 and X-10 partly.
- **B:** B-2, B-4 and B-9 resolved; B-1, B-3, B-5, B-7 and B-8 partly; B-6 not.
- **C:** C-1, C-3, C-4 and C-5 resolved; C-2, C-6 and C-7 partly.

### Defects in the code, found by this round

| From | How verified | Defect | Fixed |
| --- | --- | --- | --- |
| S2-2 | E | Renaming an address and keeping its path, with no `moved`, deleted the file and adopted the new resource from the reading taken before the removal. The run ended with no file and a record saying there was one. | `842abcd`, `INV-PLAN-009` |
| S2-10 | R, E | Rousing sent a plain restart, which starts a stopped service, while the comments and the architecture said it stays down. | `ae25cac`, `INV-REACT-003`; shown on the Gentoo test machine |
| S2-10 | R | A roused service whose own attributes changed was one `Update` that never restarted it, and the fresh record cleared the debt. | `4db7ffd`, `INV-REACT-004` |

True today and not fixed. S-3: absence is not kept as a premise, so a file that
appears between reading and applying is overwritten; a create carries no premise.
P2-11: `Failed.at` indexes premises and operations alike, which only confuses
messages.

**P-6 on a machine.** Not reproduced. A package that is not installed plans as a
create and carries no premise, so it cannot be refused with `Moved`: the case
needs a second package planned as an update whose version the first one's install
changes. The attempt on the Gentoo test machine declared one that was not
installed, and stopped earlier anyway, because its binary host has no package for
the first. The defect is narrower than first stated.

### Blockers

| Id | How verified | Finding | Resolution |
| --- | --- | --- | --- |
| C2-1 | R | `Prepared` has one set of attributes. A replacement needs what state recorded, to remove the old one, and the settled values, to make the new one. | `Prepared` carries what state recorded as well, for replacements and removals; one `prepare` per change, with a record point between the halves |
| B2-1 | E | State is rewritten whole on every write. Reproduced: 13 ms at 100 resources, 20 ms at 1000, 60 ms at 10 000 (3.5 MB) and 538 ms at 100 000 (35.6 MB), with loading at 842 ms. At the fleet size 0031 aims at, one record costs more than a `Perform`. | **Decision needed** |
| P2-1 | R, D | Every way out of a run releases the state lock while executors may still be inside a change: Ctrl-C twice removes it, and a bounded wait returns normally. The next run then reads a machine that is still moving. | **Decision needed** |

### P: protocol and failures

| Id | Severity | How verified | Finding | Resolution |
| --- | --- | --- | --- | --- |
| P2-2 | major | E, R | The frozen bootstrap has no recognition rule. Today's decoders accept unknown fields, so a bootstrap decoder tried first reads a later frame as a subset. The replacement sequence freezes far more than `Build` | A disjoint envelope decoded exactly; list what is frozen, with byte vectors and adversarial tests |
| P2-3 | major | R, D | §5.6 does not place an unasked `Stopped`, a stream protocol error, an expired wait or an endless `Working` | A table of every final frame, reason and stream error: failure, unreachable or contract violation; conversation kept or ended; pending entries kept or cleared |
| P2-4 | major | R, D | Under `continue`, the rest of a batch after `Failed` is undefined, and resending it meets the premise checked once per batch | Resend the remainder without premises and widen the bounded risk; map `Moved` to the changes owning the premise |
| P2-5 | major | D | Liveness expiry is not sticky: a `Perform` delivered after silence restarts the clock and runs | Once silence passed `L` or input ended, the conversation is over; checks run after the premise as well |
| P2-6 | major | R, D | Resolving by reading creates again what a machine or a cloud mints; "never timed out" contradicts both Ctrl-C paths | Never recreated from a pending entry without a client token or a person; say what the bounded wait does to control-domain changes |
| P2-7 | major | R, D | One thread of a three-thread executor can die and leave the conversation alive | The executor exits when any thread ends |
| P2-8 | major | R, D, E | Several failure-table rows are wrong, and events the design introduces have none | Correct them; add protocol errors, unasked `Stopped`, `conclude` refusing, the write-ahead failing, other signals, a version 4 executor, a dead thread |
| P2-9 | major | R, D | Only SIGINT is designed, and a separate process group changes what SIGTERM, SIGHUP, SIGTSTP and SIGTTIN do | INT, TERM and HUP alike; TSTP blocked during apply; `setsid`, so a prompt fails at once; delivery as well |
| P2-10 | minor | R, D | The stream rules miss gaps and reject a legal `Working` race | One state machine in `nerpa-proto`, shared by the writer and the validator |
| P2-11 | minor | R | No final frame for a `Perform` refused before anything runs | `Refused { seq, problem }` |
| P2-12 | minor | R, D | Nothing on the engine consumes `Working`, and X-8's stage deadline was dropped | A silence limit on the engine; `E`, `L`, `H` and `ServerAlive*` related in §8 |
| P2-13 | minor | E, R | An `Alive` before `Build` kills a version 4 executor; `Reach::ask` is both kept and removed | State the opening order; a synchronous ask over the two halves; an id on every request |
| P2-14 | minor | R, D | Writing ahead blocks a machine's thread, against "never blocks on the recorder" | A machine waits only for its own write-ahead; once per machine per stage in step 1 |
| P2-15 | minor | D | "Nothing is sent" contradicts `Stop` and `Alive`; a `Stop` can cross a `Finished` | "No `Perform` begins to be written"; a `Stop` for another `seq` is ignored |
| P2-16 | minor | R, D | Captured standard error needs a drainer, or `ssh` stalls | A bounded drainer per conversation |
| P2-17 | minor | D | An `Alive` queued behind a long operation makes a healthy batch look silent | The reader handles `Alive` and `Stop` itself |
| P2-18 | minor | R | §5.8 leans on `INV-PLAN-008` beyond its bypass | Name the bypass in §5.8's risk |
| P2-19 | minor | D | Restoring through the old conversation needs it alive across the reconnect | `Alive` on it throughout; a row for a restore that could not be sent |

### S: state and invariants

| Id | Severity | How verified | Finding | Resolution |
| --- | --- | --- | --- | --- |
| S2-1 | major | R, D | Under `continue`, refusal does not reach removals, and follows the wrong direction for them | A removal carries the removals that wait for it; the invariant is stated over those edges |
| S2-2 | major | E | Above | Fixed |
| S2-3 | major | R, D | Adopting what is found as declared hides a half-applied change reading cannot see: a package before its postinst, an undeclared mode, a restart | Resolved per kind: an observed change concludes from the reading, one that cannot be observed is sent again; an address owing a reaction resolves through its debt |
| S2-4 | major | R | A pending entry that reading finds matching and recorded has nothing to clear it | **Decision needed** |
| S2-5 | major | R, D | What a pending entry holds is unspecified, and an address with a time cannot find an orphan | Address, kind, the locator sent, edges, time and the record point passed; read at that locator |
| S2-6 | major | R, D | Minted identity is created again; control-domain and one-at-a-time changes get no entry | An entry for everything sent; a client token, or a refusal until a person decides |
| S2-7 | major | R | A record point says where, not what is recorded or what becomes of the entry | Points as index and what; the entry survives until the final record |
| S2-8 | major | R | The property for interrupted runs is false as written | Stated per address, with pending entries and a superset of debts |
| S2-9 | major | D, R | Only half of the reworded `INV-STATE-005` can be kept | Split: nothing is sent before a durable write names it pending; clearing is its own invariant; latency a stated bound |
| S2-10 | major | R, E | Above | Fixed |
| S2-11 | minor | R, D | Under `continue` the postcondition in `apply` panics, and `Applied` has nowhere to put refusals | Changed, failed and refused add up to the plan; `Applied` carries them |
| S2-12 | minor | D, R | Debts and pending entries are written for changes never sent | Withdrawn in the write that records the stop or refusal |
| S2-13 | minor | D | The rest of a batch after `Failed` or `Moved` is undefined | With P2-4 |
| S2-14 | minor | D, R | A second Ctrl-C releases the lock while executors act | With P2-1 |
| S2-15 | minor | R, D | The replacement plan conflicts with `INV-REACH-001` and `INV-REACH-003`, and with the stop invariant | That machine's resources listed as unread; restoring named as an exception |
| S2-16 | minor | R | How a pending removal resolves is undefined | The removal concludes from the reading, then is forgotten or planned again |
| S2-17 | nit | R | The migration from state format 4 is one-way | Say so; a downgrade that refuses while anything is pending |
| S2-18 | nit | R, D | The invariant for applying has no id; the in-doubt list's place in the plan's number is unsaid | An id, bypass and test; a stored time, never an age |

### X: security of self-replacement

| Id | Severity | How verified | Finding | Resolution |
| --- | --- | --- | --- | --- |
| X2-1 | major | R, E | The ownership and label checks cannot be expressed: what the executor reports carries mode, content and target, no owner and no label | Owner and label reported, inside the bootstrap |
| X2-2 | major | E | `mv -Tf --` and `ln -nf --` protect only the last component: reproduced, a symlinked parent is followed, and a symlink `P` gives a symlink `P.previous` | No symlink anywhere on the path, checked by the executor walking without following; `P` a regular file |
| X2-3 | major | D | A compromised executor performs and reports every step of its own replacement | Said: replacement is not recovery for a suspect executor, which stays a reinstall by root |
| X2-4 | minor | D, R | The trigger is a digest the machine reports about itself | Full digests; the trigger is the far end's choice, bounded to genuine builds |
| X2-5 | major | D | Fleet settings change what a run may do without the command line showing it | The plan's header shows the failure policy and every machine allowed to replace; **decision needed** on a command-line flag for a first replacement |
| X2-6 | minor | D, E | The health check does not prove a start through the transport, and restoring needs the old conversation | A probe through the transport after the switch; a row for a restore that could not be sent |
| X2-7 | minor | D | The bootstrap is a permanent decoding surface with no framing | With P2-2 |
| X2-8 | minor | D | Cleaning up `.new` and `P.previous`; two replacements at once | Bounded retention; `P`'s content pinned in the premise |

### B: performance

| Id | Severity | How verified | Finding | Resolution |
| --- | --- | --- | --- | --- |
| B2-1 | blocker | E | Above | **Decision needed** |
| B2-2 | major | R, D | Whether writing ahead shares group commit is unsaid; opening a stage could pay one write per machine | It shares; measured under concurrent writers |
| B2-3 | major | R, D | A limit per first hop inside the pool strands workers on a saturated hop | A slot per route taken before a worker commits to a machine |
| B2-4 | major | R | The memory row uses 8 MiB where one change may carry `MAX_CONTENT`, and a change's total size is not bounded while planning | Row corrected; a change's encoded total bounded while planning |
| B2-5 | major | E, D | Liveness tests on real timers cost seconds each | An injectable clock, or a slower layer for real timers |
| B2-6 | minor | R | Nothing holds the numbers about applying | A gate for recorder throughput and the simulated fleet |
| B2-7 | minor | R, D | Nothing says what step 1 alone buys | Step 1 measured on its own, before and after |
| B2-8 | nit | E | 100 against 1000 resources sits inside the fsync floor | Cite 10 000 and above |
| B2-9 | minor | D | The recorder's backlog is unbounded under load | A bound, and pending entries said to grow with its lag |

### C: the provider contract

| Id | Severity | How verified | Finding | Resolution |
| --- | --- | --- | --- | --- |
| C2-1 | blocker | R | Above | As above |
| C2-2 | major | R | A service's removal ignores what was recorded and settles it stopped, against "for a removal, what state recorded" | With C2-1: a removal is given both, and the provider decides per kind |
| C2-3 | major | R, D | Handing `conclude` the same `Prepared` keeps secret values alive across threads; 0027 is not among what this touches | `conclude` never sees secret values; 0027 and `INV-SECRET-*` named |
| C2-4 | major | D | Being one at a time does not let a provider branch | The claim dropped; branching splits across stages |
| C2-5 | minor | R, D | `Prepared.kind` can hold kinds that never reach a provider | A narrower kind |
| C2-6 | major | R | Replay tests are concrete for files only; the provider has no service test and one package test that only reads | Files and directories promised; a stateful fake for packages and services specified first |
| C2-7 | major | R | A `conclude` refusal follows operations that already ran, and the failure table has no row for it | A row, and §5.6 says so |
| C2-8 | minor | D | Facts are decoded twice per change | Accepted as bounded |
| C2-9 | nit | D | Which provider errors the policy governs is unsaid | Every one; what always stops is internal to the engine |

### Decisions needed before revision 2

1. **The state format** (B2-1): stop rewriting it whole, as shards per machine or a
   log with compaction, or state the fleet size where this design stops helping.
2. **The lock after a stop** (P2-1, S2-14): keep it, naming the machines and
   changes in flight, and refuse those machines until opening shows nothing
   running there or a person unlocks.
3. **A first executor replacement** (X2-5): whether it needs a command-line flag
   as well as the fleet's opt-in.
4. **Clearing a pending entry that matches and is recorded** (S2-4): a kind that
   confirms it, so the first run after an interrupted one is not empty, or treating
   the address as unrecorded, so it is adopted.

### Revision 2, 2026-09-11

What changed after the second review, finding by finding, and the owner's four
decisions. The defects in code this round found are fixed above.

| Findings | Answered in | How |
| --- | --- | --- |
| C2-1, C2-2 | §5.2 | `Prepared` carries what state recorded; a replacement is one `prepare` with a `Forget` point; a removal is given both, and its meaning is decided per kind |
| C2-3 | §5.2 | `Concluding` holds no secret value; 0027 and `INV-SECRET-*` among what this touches |
| C2-4 | §5.1, §5.2 | Being one at a time is about order only |
| C2-5 | §5.2 | `Sending` |
| C2-6 | §5.2, §5.12 | Replay tests for files and directories; a stateful fake first for packages and services |
| C2-7 | §5.2, §5.6, §5.13 | A refusal from `conclude` follows operations that ran |
| C2-8 | §5.2 | Accepted |
| C2-9 | §5.2, §5.6 | Every `ProviderError` governed by the policy |
| B2-1 | §5.7 | Owner's decision: state in shards per machine |
| B2-2 | §5.5, §5.7 | Writing ahead shares the shard's group commit |
| B2-3 | §5.5, §5.11 | A route slot taken before a worker commits to a machine |
| B2-4 | §5.9, §5.11 | A change's size bounded while planning; the memory row corrected |
| B2-5 | §5.12 | A clock the tests control |
| B2-6, B2-7 | §5.12 | A gate for recorder throughput and the simulated fleet; step 1 measured on its own |
| B2-8 | §5.7 | The figures at 10 000 and 100 000 cited, and the fsync floor below 1000 said |
| B2-9 | §5.5 | A machine sends its next batch only after its last is recorded |
| P2-1, S2-14 | §5.5, §5.13 | Owner's decision: the lock kept, naming the machines still working |
| P2-2, X2-7 | §5.3 | A disjoint envelope decoded exactly, and what is frozen listed |
| P2-3 | §5.6 | A table of every outcome |
| P2-4, S2-13 | §5.6, §5.8 | The rest of a batch sent again without premises; `Moved` refuses only the changes owning the premise |
| P2-5 | §5.3, §5.4 | An executor that has stopped stays stopped; checks after the premise too |
| P2-6, S2-6 | §5.2, §5.7 | Minted identity found by token or refused, never made twice |
| P2-7 | §5.4 | The executor exits when a thread ends |
| P2-8 | §5.13 | Rows corrected and added |
| P2-9 | §5.5 | INT, TERM and HUP alike; TSTP blocked; `setsid`; delivery too |
| P2-10 | §5.3 | One state machine for the stream |
| P2-11 | §5.3 | `Refused`; `Failed.at` names a premise or an operation |
| P2-12 | §5.3, §8 | `L_e` on the engine, and the timers related |
| P2-13 | §5.3, §5.5 | The opening order; an id on every request; `Reach::ask` kept over the two halves |
| P2-14 | §5.5 | A machine waits for its own write-ahead only, once per stage |
| P2-15 | §5.3, §5.6 | "No `Perform` begins to be written"; a `Stop` for another `seq` ignored |
| P2-16 | §5.3, §5.11 | A bounded drainer |
| P2-17 | §5.4 | The reader handles `Alive` and `Stop` |
| P2-18 | §5.8 | The bypass named in the bounded risk |
| P2-19 | §5.10, §5.13 | `Alive` on the old conversation throughout; a row for a restore not sent |
| S2-1 | §5.6 | A removal carries the removals that wait for it |
| S2-3, S2-4 | §5.2, §5.7 | Owner's decision: written ahead and sent again, or found by token, only where reading cannot confirm; nothing waits on a confirmation, so S2-4 does not arise |
| S2-5 | §5.7 | What an entry holds |
| S2-7 | §5.2, §5.7 | `Forget` and `Partial` points; the entry survives them |
| S2-8 | §5.7 | The property restated per address |
| S2-9 | §5.7 | `INV-STATE-005` split; latency a stated bound |
| S2-11 | §5.6 | The postcondition, and `Applied` carrying failures and refusals |
| S2-12 | §5.6, §5.7 | Entries withdrawn for what was never sent |
| S2-15 | §5.10 | The machine's resources listed as unread; restoring a named exception |
| S2-16 | §5.7 | A pending removal sent again |
| S2-17 | §5.7 | One-way, with a downgrade that refuses while anything is pending |
| S2-18 | §5.6, §5.7 | `INV-APPLY-001`; the time an entry was sent kept as a time, outside the plan's number |
| S-3 | §5.8 | Absence is a premise |
| X2-1, X2-2 | §5.10 | The executor walks the path without following links and reports owner, mode and label |
| X2-3 | §5.10 | The risk stated both ways |
| X2-4 | §5.10 | Full digests; the trigger is the far end's choice, bounded to genuine builds |
| X2-5 | §5.6, §5.10 | Owner's decision: the fleet's opt-in and a flag; the plan's header shows the policy and the machines |
| X2-6 | §5.10, §5.13 | A probe through the transport before a replacement counts |
| X2-8 | §5.10 | One `P.previous`; `P`'s content in the premise |

A third review is pending.

## Third review, 2026-09-11

The same five angles and models, on revision 2 (`d6644b1`). Every finding was
checked before it was logged. The reviewers for P and B ran experiments of their
own: P against the executor through pipes, B against the store's write and sync on
this control machine.

**The second round, as revision 2 left it.**

- **P:** P2-5, P2-11, P2-15, P2-16, P2-17 and P2-18 resolved; the rest partly.
- **S:** S2-2 and S2-10 resolved; the rest partly.
- **X:** X2-3 resolved; the rest partly.
- **B:** B2-8 resolved; the rest partly.
- **C:** C2-1, C2-2, C2-4, C2-5, C2-7, C2-8 and C2-9 resolved; C2-3 and C2-6 partly.

### Defects in the code, found by this round

| From | How verified | Defect | Fixed |
| --- | --- | --- | --- |
| S3-7 | R | Removals were planned from state without asking whether their machine answered. A record on a machine that did not answer was planned for removal in a plan that called itself complete, and apply stopped at that first stage, before touching the machines that did answer. | `d7330a9`, `INV-REACH-001` |

True today and not fixed. X3-13: a frame is decoded as one CBOR item with nothing
checking that the frame ends there, so trailing bytes are accepted; harmless while
frames are length-delimited. P3-7: the executor returns at the first premise that
moved, so later premises are never evaluated, which is right for one change and
matters only for batches.

### Blockers

| Id | How verified | Finding | Resolution |
| --- | --- | --- | --- |
| C3-1 | R | The executor answers a file write with the file's content, so `conclude` would be handed a secret's value, against revision 2's "`conclude` never sees a secret value". | **Owner's decision:** a write whose content is secret is answered with a digest sealed for the machine, never the content. A protocol change, one reinstall, and a wider closed operation set (0033) |
| B3-1 | E, R | "The recorder is the only writer of state (0031)" and "machines write in parallel" say opposite things. Measured on this ext4: the store's write-sync-rename-sync on one small file per thread gives 75 writes a second on one thread and 923 on sixteen. | A writer per shard under one recorder that owns ordering; 0031's serialisation point corrected (RFC 0001) |
| B3-2 | D | Three threads per open conversation for the whole run scale with the machines carrying work, not with the pool. | Belongs to streamed batches: step 1 keeps one synchronous conversation per machine in flight (RFC 0003) |
| P3-12 | D | An absence premise on a create is moved by an earlier change of the same run on that machine, such as a package laying down the configuration file declared after it, so a fresh machine's first run fails. | **Owner's decision:** no absence premise; a create writes over what appeared between reading and writing, as today, and the risk is stated (RFC 0001) |

### P: protocol and failures

| Id | Severity | How verified | Finding | Goes to |
| --- | --- | --- | --- | --- |
| P3-1 | major | R, D | One lock file cannot both keep out a live run and name machines still being acted on; a killed engine never rewrites it; "nothing of that run running there" needs a run identity protocol 4 lacks | RFC 0001: a lock the kernel releases, an in-flight record written before each request, a person releasing machines in step 1 |
| P3-2 | major | R, D | A panic, including a failed write to a closed terminal, unwinds into the lock guard and deletes the lock while executors act | RFC 0001: a panic hook that stops the run; the in-flight record is never removed on unwind |
| P3-3 | major | E, R | A version 4 executor exits on the bootstrap envelope without answering, so step 2 starts with one install by hand | RFC 0002 |
| P3-4 | major | D, E, R | The frozen list lacks `Alive`, ids, the frame layer and premise machinery, and freezes a general process runner | RFC 0002 |
| P3-5 | minor | E, R | "Decoded exactly" is undefined at the byte level, and the stamp-first read is itself a partial read | RFC 0002 and 0003 |
| P3-6 | major | D, R | Engine-side liveness is undefined while a conversation is idle, and the timers count frames where they need bytes | RFC 0003 |
| P3-7 | major | E, R | After `Moved`, resending without premises applies changes whose premises were never evaluated | RFC 0003 |
| P3-8 | major | R, D | Under `continue`, the outcome table withdraws entries of changes it then resends | RFC 0001 for step 1's table; RFC 0003 for resends |
| P3-9 | minor | R, E, D | `Refused` semantics are inverted | RFC 0003 |
| P3-10 | major | R, D | The stream rules make an expired executor's `Stopped` a contract violation, and miss constraints | RFC 0003 |
| P3-11 | major | R, D | Exiting when any thread ends splits a change at end of input | RFC 0003 |
| P3-12 | blocker | D | Above | RFC 0001 |
| P3-13 | minor | R | In step 1, one `Perform` for a replacement makes `Failed.at` decide state | RFC 0001 |
| P3-14 | minor | R | Defects in the failure table | RFC 0001 for step 1's rows; RFC 0003 |
| P3-15 | minor | D, R | Signal details: a HUP followed by TERM, blocked TSTP, SIGSTOP, askpass prompts, step 1 without `Stop` | RFC 0001 |
| P3-16 | minor | R | §5.5 contradicts itself on waiting for records | RFC 0001 |
| P3-17 | minor | R, D | Keeping `Reach::ask` needs exclusivity stated, and the failure classes while reading | RFC 0001 |
| P3-18 | nit | R | `seq` against "id"; a test path; which end of stderr is kept | RFC 0003 |

### S: state and invariants

| Id | Severity | How verified | Finding | Goes to |
| --- | --- | --- | --- | --- |
| S3-1 | major | R, D | A debt can sit in another shard from its cause, whose write goes first | RFC 0001 |
| S3-2 | major | R, D | Where the single digest key lives among shards, and how a file becomes a directory without a moment of no state | RFC 0001 |
| S3-3 | major | R, D | "Sent again" has no place in the planner: a pending package that reads as installed is adopted | RFC 0001 |
| S3-4 | major | R, D | On rhel, sending an install again runs no post-install; `dnf reinstall` does | RFC 0001 |
| S3-5 | major | R, D | Nothing says what a pending entry becomes when its declaration changed or went | RFC 0001 |
| S3-6 | major | R, D | Option D brings back unowned files for what a shard has not yet written, and lost reorders keep old edges | RFC 0001 |
| S3-7 | major | R | Above | Fixed |
| S3-8 | major | R, D | The removal edges for `INV-APPLY-001` point the wrong way, and two are missing | RFC 0001 |
| S3-9 | major | D | The outcome table's entry column contradicts `continue` | RFC 0001 |
| S3-10 | major | R, D | A dead run's kept machines and a live run's exclusion cannot share one lock file | RFC 0001, with P3-1 |
| S3-11 | major | R, D | Absence as a premise changes `read` and breaks creates after removals | Moot: no absence premise |
| S3-12 | minor | D | The clearing invariant contradicts §5.7 | RFC 0001 |
| S3-13 | minor | R, D | The interrupted-run property misses records made to carry a debt | RFC 0001 |
| S3-14 | minor | R | The postcondition leaves out changes never begun and outcomes unknown | RFC 0001 |
| S3-15 | minor | R | Shard names: `.`, `..` and names differing only in case are valid targets (verified in `address.rs`) | RFC 0001 |
| S3-16 | minor | R, D | No stage barrier across shards | RFC 0001 |
| S3-17 | minor | D | "The second run after an interrupted one is empty" has unlisted exceptions | RFC 0001 |
| S3-18 | minor | R, D | Whether debts for causes never sent are withdrawn contradicts itself | RFC 0001 |
| S3-19 | nit | R | "Reading sees everything" holds only for declared attributes | RFC 0001 |

### X: security of self-replacement

| Id | Severity | How verified | Finding | Goes to |
| --- | --- | --- | --- | --- |
| X3-1 | major | R, D | The frozen set cannot carry the sequence: `Alive`, removing `.new`, a digest, the restore | RFC 0002 |
| X3-2 | major | R, D | Generic run, link and rename frozen for ever; narrow them to the sequence | RFC 0002 |
| X3-3 | major | D | Nothing binds the operations to what was walked | RFC 0002 |
| X3-4 | major | D | Owner, mode and label cannot decide "writable by nobody else": groups, ACLs, mounts, attributes | RFC 0002 |
| X3-5 | major | D | No automatic rollback after the switch | RFC 0002 |
| X3-6 | major | R, D | Unwinding after a panic removes the lock | RFC 0001, with P3-2 |
| X3-7 | major | R, D | The plan shows replacements only while they happen, and the flag authorises every opted-in machine | RFC 0002 |
| X3-8 | major | R, D | The first replacement cannot run through protocol 4 | RFC 0002 |
| X3-9 | minor | D | Which part of a label is compared | RFC 0002 |
| X3-10 | minor | R, D | `P`'s content is never tied to what answered `Build` | RFC 0002 |
| X3-11 | minor | R | Fleet group keys are values for the configuration, and unknown node keys are ignored (verified in `fleet.rs`) | RFC 0001 for `[apply]`; RFC 0002 |
| X3-12 | minor | D | A replacement is not a unit for stopping or for the lock | RFC 0002 |
| X3-13 | minor | R | The rewording of `INV-WIRE-001` drops half of it; trailing bytes accepted today | RFC 0002 |
| X3-14 | minor | D | What makes a build a release | RFC 0002 |
| X3-15 | minor | D | `P.previous` keeps an older build beside `P` for good | RFC 0002 |
| X3-16 | minor | D | The risk statement is incomplete | RFC 0002 |
| X3-17 | nit | R, D | Umask on create, digest length in names, empty path parts, where `arch` comes from | RFC 0002 |

### B: performance

| Id | Severity | How verified | Finding | Goes to |
| --- | --- | --- | --- | --- |
| B3-1 | blocker | E, R | Above | RFC 0001 |
| B3-2 | blocker | D | Above | RFC 0003 |
| B3-3 | major | R, D | `fanout::every` cannot skip a saturated route | RFC 0001 |
| B3-4 | major | E, R | The backlog bound turns recorder throughput into latency on each machine's path | RFC 0001 |
| B3-5 | major | R, D | The change-size bound has no number, and a write's answer echoes its content, doubling memory | RFC 0001 |
| B3-6 | minor | R | The "stated bound" on reaching disk is not stated | RFC 0001 |
| B3-7 | minor | E, R | Figures carry no statistic; the reviewer's medians differ 1.3–2.6× from ours | RFC 0001 |
| B3-8 | minor | R | The stderr buffer has no size | RFC 0001 |
| B3-9 | minor | D | The Teleport limits carry no citation | RFC 0001 |
| B3-10 | nit | E | Shard count costs nothing at these scales (2000 files listed in 0.4 ms) | RFC 0001 |

### C: the provider contract

| Id | Severity | How verified | Finding | Goes to |
| --- | --- | --- | --- | --- |
| C3-1 | blocker | R | Above | RFC 0001 |
| C3-2 | major | R, D | `Records` has no value for a change that completed | RFC 0001 |
| C3-3 | major | D | Nothing says what is recorded when an interruption falls between points | RFC 0001 |
| C3-4 | major | R | `Sending` and `#[non_exhaustive]`: which trade-off | RFC 0001 |
| C3-5 | major | R, D | A package's reaction is undefined: nothing on rhel, a re-merge on Portage | **Owner's decision:** packages cannot react; refused when configured |
| C3-6 | minor | R, D | `afterwards` does not say a rousing is covered by its debt | RFC 0001 |
| C3-7 | minor | R, D | `one_at_a_time` is handed secret values it does not need | RFC 0001 |
| C3-8 | minor | D | The record-point example is the one case where points change nothing | RFC 0001 |
| C3-9 | major | R, D | There is no sealed answer for an operation, only for an observation | With C3-1 |

### Decided by the owner after the third review

1. **Split this RFC.** Step 1 stays here, revised and reviewed narrowly before
   acceptance. Replacing an installed executor moves to RFC 0002, not scheduled,
   with its findings. Streamed batches move to RFC 0003, to be written once step 1
   is built and measured. The history stays in this log.
2. **A write whose content is secret is answered with a sealed digest,** never the
   content: a protocol change and one reinstall.
3. **No absence premise.** A create writes over what appeared between reading and
   writing, as today, and the risk is stated.
4. **Packages cannot react.** `reacts_to` on a package is refused when configured.

### Revision 3, 2026-09-11

Narrowed to step 1, by the owner's decision. What changed after the third review,
finding by finding, for everything that stayed; what moved is marked in the tables
above.

| Findings | Answered in | How |
| --- | --- | --- |
| P3-1, S3-10 | §5.8 | `lock` held with `flock`; `in-flight` written before each request; a person releases machines |
| P3-2, X3-6 | §5.8 | A panic hook stops the run; nothing on unwind removes `in-flight`; reports written with errors ignored |
| P3-8, S3-9 | §5.5 | One entry column, withdrawn only where nothing ran; no resends within a run |
| P3-12, S3-11 | §5.7 | Owner's decision: no absence premise |
| P3-13 | §5.1, §5.3 | A `Perform` per point; a failure names a premise or an operation |
| P3-14 | §5.12 | Rows for this RFC |
| P3-15 | §5.8 | Signals, a grace interval, TSTP ignored, SIGSTOP and askpass stated |
| P3-16 | §5.4 | A machine waits for its own write-ahead and previous record, never another's |
| P3-17 | §5.3 | A conversation used exclusively; the classes while reading |
| S3-1 | §5.4 | A cause waits for its reactor's shard write |
| S3-2 | §5.6 | A `key` file first; a migration renamed into place |
| S3-3, S3-5 | §5.6 | A rule per entry and declaration |
| S3-4 | §5.2 | `resent`, and `dnf reinstall` |
| S3-6 | §5.6 | The bounded risk: the change in flight per machine |
| S3-8 | §5.5 | Four kinds of edge, in `depends_on` and the plan's number |
| S3-12 | §5.6 | The clearing invariant reworded |
| S3-13 | §5.6 | Four cases; "as read or reported" |
| S3-14 | §5.5 | Five outcomes on every way out |
| S3-15 | §5.6 | Shards named by the hexadecimal of the target |
| S3-16 | §5.4 | A stage barrier |
| S3-17 | §5.6, §5.11 | "The run after the one that recovers", with its exceptions, and a property |
| S3-18 | §5.5, §5.6 | Debts never withdrawn |
| S3-19 | §5.2 | "Every declared attribute" |
| X3-11 | §5.5 | Settings in a table of their own, unknown keys refused |
| B3-1 | §5.4 | A writer per shard under one recorder; 0031's serialisation point corrected |
| B3-3 | §5.4 | Queues per route |
| B3-4 | §5.4 | What a machine waits for, with figures |
| B3-5 | §5.3, §5.9 | Digests in answers; a change bounded at `MAX_CONTENT`; memory stated |
| B3-6 | §5.6 | The bound: the next write of the shard |
| B3-7 | §5.6 | Medians of one session, and the other session's difference |
| B3-8 | §5.3, §5.10 | The last 64 KiB |
| B3-9 | §5.10 | Marked as from Teleport's documentation, not measured |
| B3-10 | §5.6 | Stated |
| C3-1, C3-9 | §5.2, §5.3 | Owner's decision: write answers as digests, sealed for secrets |
| C3-2 | §5.2 | `Option<Point>` |
| C3-3, C3-8 | §5.2 | The highest point passed; points matter only where reading cannot confirm |
| C3-4 | §5.2 | `#[non_exhaustive]`, and an unknown kind refused |
| C3-5 | §5.2 | Owner's decision: packages cannot react, built |
| C3-6 | §5.2 | Rousing covered by its debt |
| C3-7 | §5.2 | `one_at_a_time` takes `Concluding` |

The narrow review follows.

## Narrow review, 2026-09-11

Four of the five angles, on revision 3 (`6f5a6ed`) and step 1 only: P and S by
Opus, B and C by Sonnet. X was not repeated, because what it reviewed moved to RFC
0002. Every finding was checked before it was logged. The reviewer for P ran
experiments of its own against the executor through pipes. The reviewer for B ran
none and cites the runs logged for the second and third reviews.

**The third round, as revision 3 left it.**

- **P:** P3-8, P3-13 and P3-17 resolved; P3-1, P3-2, P3-14 and P3-15 partly; P3-16
  not.
- **S:** S3-9, S3-10, S3-13, S3-14, S3-15, S3-18 and S3-19 resolved; S3-1 to S3-6,
  S3-8, S3-12, S3-16 and S3-17 partly.
- **B:** B3-5, B3-6, B3-8, B3-9 and B3-10 resolved; B3-1, B3-3, B3-4 and B3-7 partly.
- **C:** C3-2, C3-5, C3-6 and C3-7 resolved; C3-1, C3-3, C3-4, C3-8 and C3-9
  partly.

### Defects in the code, found by this round

| From | How verified | Defect | Fixed |
| --- | --- | --- | --- |
| S4-3 | R, E | Removals came first and waited only for other removals. A file recorded after its directory and moved out of it while the directory's declaration went was still inside when the directory was removed; the removal refused, and every later run did the same. | `3d2d383`, `INV-PLAN-011` |
| S4-1 | R, E | A cause on a machine that answered was sent while the resource reacting to it was on one that did not, or held behind one, and no debt was written: when that machine came back the file matched, and nothing was owed. | `6e5bec8`, `INV-REACT-006`; the rule decided by the owner |
| — | R, E | Found while fixing S4-3. A declared resource naming what a held removal takes away was adopted from the reading, and the removal, planned in a later run, took the thing away from under that record. | `1e04f62`, `INV-PLAN-010` |

Latent and not fixed. P4-2: a private process answers with its output, and a
failure quotes its standard error despite "(input withheld)". No operation of the
`os` provider is private today; revision 4 answers it with protocol 5.

### Blockers

| Id | How verified | Finding | Resolution |
| --- | --- | --- | --- |
| B4-1, S4-6, P4-5 | R, D | `in-flight` is one file beside `lock` in §5.6, and is written through each machine's group commit in §5.8. Several shard writers would rewrite one file and lose a machine's mark, or a single writer would put a durable write before every request of every machine. §5.4 counts neither write. | The mark is a field of each machine's shard, written with the record of the change before it: one durable write per `Perform`, and no shared file |
| C4-1, S4-10, P4-1 | R, D | `conclude` cannot compare a sealed write digest with what it asked for. It holds no secret value and never sees the seal, and nothing else compares either. | The engine holds the value and the seal when it seals a `Perform`. It computes the expected digest and the state's digest there, and compares the answer itself; `conclude` is told that the write matched |
| C4-2, S4-5 | R, D | A resend cannot always be `dnf reinstall`, which fails for a package that is not installed, and `prepare` cannot branch. | A change is marked resent only when this run's reading finds the resource; otherwise it is an ordinary install |

### P: protocol and failures

| Id | Severity | How verified | Finding | Goes to |
| --- | --- | --- | --- | --- |
| P4-1 | major | R, D | With C4-1. The recorder cannot make a written secret's state digest either, and `conclude` cannot report the attribute back as `INV-PLAN-005` requires. | Blocker, above |
| P4-2 | major | E, R | A private process answers with its output, and a failure quotes its standard error despite "(input withheld)". A value derived from a secret comes back the same way. Measured: `cat` with private input answered with that input. | §5.3: a private process answers with its status and a digest of its output sealed for the machine, and a problem never quotes it |
| P4-3 | major | R, D | A stop splits a change. A replacement whose delete was answered and recorded is never created, on a machine that did not fail, and the postcondition has no place for it. | §5.8: stops end at change boundaries |
| P4-4 | major | R, D | Entries written ahead for a whole stage and never sent are resent by a later run, and a resent install on rhel fails. | §5.6: an entry is written with its change's mark, so none exists for a change never sent |
| P4-5 | major | R, D | With B4-1. Setting and clearing the mark are two writes, and "exactly the machines" is false. | Blocker, above; "at least" |
| P4-6 | major | R, D | A stop cannot end a blocked read, because the child lives inside the conversation's mutex. A recorder that panics leaves workers waiting, and `thread::scope` never returns. | §5.8: a kill handle outside the mutex; every wait watches the stop flag |
| P4-7 | major | R, D | One request that never answers holds every machine's next stage through the stage barrier. | §5.4: waits per edge in place of the barrier |
| P4-8 | major | R, E, D | Route slots bound applying, which reuses connections, while `MaxStartups` counts the handshakes made at opening. The fleet has no first hop; only `ssh -G` knows it. | §5.4, §5.10: slots bound opening; the route key from `ssh -G` |
| P4-9 | minor | E, R | A version 4 executor exits on a version 5 frame without answering, so "Refused by `Build`" is wrong, and the reason is lost among handshakes. | §5.3, §5.12 |
| P4-10 | minor | E, R | `Failed` has no arm for a refusal before anything ran. Nothing refuses a secret write without a seal, unknown keys decode, and "a digest of what was written" is not defined. | §5.3 |
| P4-11 | minor | E, R, D | "`ssh` failed before authenticating" cannot be observed: exit 255 covers every `ssh` error, and standard error can be empty. | §5.3: retried on exit 255 with no byte read and only `Build` written |
| P4-12 | minor | R, D | Rows missing or wrong in §5.5 and §5.12: `Done` at a point, an answer of the wrong kind or count, an executor exiting on a request it cannot decode, `Route::Here`. | §5.5, §5.12 |
| P4-13 | minor | R, D | `lock` must never be unlinked, `release` and the downgrade must take it, and versions 4 and 5 lock different files during a migration. | §5.8, §5.13 |
| P4-14 | minor | E, R | A signal blocked or ignored in the engine reaches every child. Measured in the `SigBlk` and `SigIgn` of a program the executor ran. | §5.8 |
| P4-15 | minor | R, D | Preparing a stage whole holds every operation and secret of the stage, against the memory bound and 0027. | §5.2, §5.9: a change prepared just before its `Perform` |
| P4-16 | nit | D | Reports written beside state have no name that loading ignores, and merging HUP with TERM swallows a second Ctrl-C. | §5.8 |

### S: state and invariants

| Id | Severity | How verified | Finding | Goes to |
| --- | --- | --- | --- | --- |
| S4-1 | major | R, D | A cause is sent while its reactor is not in the plan, so no debt is written. | Built; see the defects above |
| S4-2 | major | R, D | Edges carried in `depends_on` for the failure policy would be recorded, and the next plan would be a reorder. | §5.5: `waits_for`, hashed and shown, never recorded |
| S4-3 | major | R, D | An edge is missing from the planner and from §5.5: the removal of X after the change of a declared Y whose record names X. | Built; see the defects above |
| S4-4 | major | R, D | The entry table can plan an adoption, a reorder or nothing, which would clear an entry without sending anything, or never clear it. "Changed" and the point passed are undefined. | §5.6: an entry always yields a change that sends something |
| S4-5 | major | D, R | With C4-2. | Blocker, above |
| S4-6 | major | R, D | With B4-1. The mark holds less than an entry, and what `release` makes of it is unsaid. | Blocker, above; §5.8 |
| S4-7 | major | R | Splitting `INV-STATE-005` drops "a machine sends its next change only after its previous one is recorded", which the bounded risk rests on. | §5.6: a third clause |
| S4-8 | major | D | The interrupted-run property is false for a change never begun, a moved locator and a `Forget` point. | §5.6: stated per change |
| S4-9 | major | R, D | A directory cannot be renamed onto the state file, and the other order leaves a moment with no state, which loads as empty. | §5.13: format 5 at a path of its own |
| S4-10 | major | R, D | With C4-1, and what state records for a file's content is undefined. | Blocker, above; §5.6 |
| S4-11 | major | R, D | Reporting a held machine as unreachable conflates the two, a plan with one has no rule for its digest, and `release` is unspecified. | §5.8: `Held` in the plan |
| S4-12 | major | R, D | §5.4 contradicts itself about waiting, and its barrier can hold the fleet without bound. | §5.4: waits per edge |
| S4-13 | major | R, D | `INV-APPLY-001` misses control-domain changes, adoptions and reorders, cannot be checked under `stop` across workers, and withdraws an entry on a `Moved` after an earlier `Perform` of the change ran. | §5.5 |
| S4-14 | major | R, D | "Read by the token" cannot be expressed in this contract, and no step-1 provider needs it. | §5.2: `FindByToken` out of step 1 |
| S4-15 | minor | R, D | Owing every reaction of a stage at its start widens 0026's window. | §5.4: owed per cause |
| S4-16 | minor | R, D | Hexadecimal doubles a target's length past `NAME_MAX`, and a shard does not say which target it holds. | §5.6 |
| S4-17 | minor | R, D | `resent` has to be encoded for every change, which changes every plan's number. | §5.6: the digest's domain goes to version 2 |
| S4-18 | minor | R, D | The outcome buckets are neither disjoint nor total. | §5.5 |
| S4-19 | nit | R, D | "One-way" sits beside a downgrade command, `.v4` holds a live key, and the lock's descriptor must not be inherited. | §5.8, §5.13 |

### B: performance

| Id | Severity | How verified | Finding | Goes to |
| --- | --- | --- | --- | --- |
| B4-1 | blocker | R, D | `in-flight` is specified two ways. | Blocker, above |
| B4-2 | major | R, D | §5.4 has a cause wait for another shard's write, and says two lines later that no machine waits for another's record. | §5.4 |
| B4-3 | major | R, D | The stage barrier gathers the whole fleet at every stage, not only shards joined by an edge. | §5.4: waits per edge |
| B4-4 | major | R, D | The route scheduler has no algorithm and nothing to group by: the fleet has no first hop. | §5.4, §5.10 |
| B4-5 | major | R | §5.9 undercounts memory: `Look` still returns content, and sending clones an ask twice beside its CBOR buffer. | §5.9 |
| B4-6 | minor | R | The performance gates have no concrete configuration. | §5.11 |
| B4-7 | nit | R | 75 and 923 writes a second carry no label. | §5.4 |
| B4-8 | nit | R, D | `MAX_CONTENT`'s reason is an answer echoing content, which protocol 5 removes. | §5.9 |

### C: the provider contract

| Id | Severity | How verified | Finding | Goes to |
| --- | --- | --- | --- | --- |
| C4-1 | blocker | R, D | `conclude` cannot check a sealed digest. | Blocker, above |
| C4-2 | blocker | R, D | A resend cannot choose between installing and reinstalling. | Blocker, above |
| C4-3 | major | R, D | `INV-PLAN-005` compares what `apply` returns, and a secret's state digest is made from the value returned, which `conclude` never holds. | §5.2 |
| C4-4 | major | R, D | A plain digest answer has no wire shape, and nothing says how it is checked. | §5.3 |
| C4-5 | major | R, D | Points past `Forget`, and `FindByToken`, are exercised by nothing step 1 ships. | §5.2 |
| C4-6 | major | R | `Reading` has no token. | With C4-5 |
| C4-7 | minor | D | A package's update runs its post-install as well, and the prose names only install and removal. | §5.2 |
| C4-8 | minor | R, D | A removal that only settles, as a service's does, has no named `Concluded`. | §5.2 |

### Decided by the owner after the narrow review

1. **S4-1's rule.** A reacting resource the plan cannot reach is owed the reaction
   on its record before its cause is sent, and one with no record holds its cause.
   Built.
2. **Revision 4, and another round of P and S** before acceptance.

### Revision 4, 2026-09-11

What changed after the narrow review, finding by finding.

| Findings | Answered in | How |
| --- | --- | --- |
| B4-1, S4-6, P4-5 | §5.6, §5.8 | The mark is part of each machine's shard: one durable write per `Perform`, and "at least" |
| C4-1, C4-3, S4-10, P4-1 | §5.2, §5.3, §5.6 | The engine computes and checks write digests; `conclude` is told a write matched; `INV-PLAN-005` restated |
| C4-2, S4-5 | §5.2, §5.6 | Resent only when the reading finds the resource |
| C4-4 | §5.3 | `Seen::Digest` and `Seen::Sealed` |
| C4-5, C4-6, S4-14 | §5.2 | `FindByToken` and partial points out of step 1; `forget_after` for a replacement |
| C4-7, C4-8 | §5.2 | Every package change is `SendAgain`; a removal that settles concludes `Gone` |
| S4-1 | §3, §5.4 | Built, `INV-REACT-006`; a cause waits for its debts in other shards |
| S4-2 | §5.5 | `waits_for`, never recorded |
| S4-3 | §5.5 | Built, `INV-PLAN-011`: the fifth edge |
| S4-4 | §5.6 | An entry always yields a change that sends something |
| S4-7 | §5.6 | `INV-STATE-005`'s third clause |
| S4-8 | §5.6 | The property stated per change |
| S4-9, S4-19, P4-13 | §5.8, §5.13 | `<state>.d/`, `format` written last, both locks taken, a refusing file at the old path |
| S4-11 | §5.8 | `Held` in the plan; `release` specified |
| S4-12, B4-2, B4-3, P4-7 | §5.4 | Waits per edge; a machine that never answers holds only what waits for it |
| S4-13 | §5.5, §5.7 | `INV-APPLY-001` over every path, after the stop flag; premises on a change's first `Perform` |
| S4-15 | §5.4 | Owed per cause |
| S4-16 | §5.6 | Shards named by a digest of the target, and each carrying it |
| S4-17 | §5.6 | Encoded for every change; the snapshot changes with it |
| S4-18 | §5.5 | One outcome for each change |
| P4-2 | §5.3 | A private process answered with sealed digests |
| P4-3 | §5.8 | Stops end at change boundaries |
| P4-4 | §5.6 | An entry exists only with its change's mark |
| P4-6 | §5.8 | Kill handles outside the mutex; waits watch the stop flag; a supervisor |
| P4-8, B4-4 | §5.4, §5.10 | Route slots at opening; the route from `ssh -G`; routes in turn |
| P4-9 | §5.3, §5.12 | The row corrected; opening quotes the refusal |
| P4-10 | §5.3 | `Before` and `Observation(n)`; a seal required; unknown keys refused; the digest of the entry read back |
| P4-11 | §5.3 | Retried on exit 255 with no byte read and only `Build` written |
| P4-12 | §5.5, §5.12 | Rows split, corrected and added |
| P4-14, P4-16 | §5.8 | A self-pipe; children with a clean mask; only HUP and TERM merged |
| P4-15 | §5.2, §5.9 | A change prepared just before its `Perform` |
| B4-5, B4-8 | §5.9 | Memory while reading stated; three copies while sending; `MAX_CONTENT`'s reason |
| B4-6 | §5.11 | A gate with its configuration |
| B4-7 | §5.4 | The rates labelled |

The fifth review follows.

## Fifth review, 2026-09-11

P and S, both by Opus, on revision 4 (`d11060f`) and step 1 only. Every finding
was checked before it was logged. The reviewer for P ran experiments against the
executor through pipes, a harness built outside the tree, and `ssh -G`. The
reviewer for S ran the planner on copies of `d11060f` and `1e04f62`.

**The narrow review, as revision 4 left it.**

- **P:** P4-4, P4-5, P4-9, P4-14 and P4-15 resolved; P4-1, P4-2, P4-3, P4-6, P4-7,
  P4-8, P4-10, P4-11, P4-12, P4-13 and P4-16 partly. Of the other angles, B4-2 and
  B4-3 resolved; B4-4, S4-12 and S4-13 partly.
- **S:** S4-10 and S4-14 resolved; every other S4 finding partly.

### Defects in the code, found by this round

| From | How verified | Defect | Fixed |
| --- | --- | --- | --- |
| S5-1 | E | A regression from `3d2d383`. Ordering a removal after the change of what state recorded after it moved it past the replacement of the directory the removed file was in: the directory was deleted with the file still inside, and every run refused. Removals first had kept that order without the planner saying so. | `684c024`, `INV-PLAN-011` |
| S5-3 | E | A directory replaced while a declared file recorded inside it moved out: one stage, the directory first, and every run refused. | `684c024`, `INV-PLAN-011` |
| S5-15 | E | The refusal of a removal between two changes could name one resource as both sides. | `684c024`: a refusal names each step of a cycle |
| S5-2 | E | A recorded file moved onto the path a removal frees was planned as a creation, because its reading was dropped; its old path stayed on the machine with no record. Since `842abcd`. | `d7b74fd`, `INV-PLAN-009` |
| P5-4 | E, R | A read interrupted by a signal inside a frame was a lost connection. Latent: no signal handler is installed yet. | `93d3912` |
| P5-6 | E, R | A file mode written other than as four octal digits never converged: "644", "0o644" and "100644" were updated on every run. Wider than the finding, which named only the last. | `0285b88`, `INV-PLAN-012` |
| S5-13, S5-14 | E | A change held on a machine that answered was reported as that machine not answering, and a held change on no machine was not reported at all. | `d78c674`, `INV-REACH-004` |

### Blockers

| Id | How verified | Finding | Resolution |
| --- | --- | --- | --- |
| S5-6, P5-1 | D | A mark is at once a pending entry and a hold on its machine. A determinate failure after operations keeps a `SendAgain` mark, so a failed package holds its machine under either policy, and `release` keeps the mark as an entry, so a machine with an interrupted package cannot be freed. | A mark carries its state, in flight or entry, and only in flight holds. A determinate answer turns it into an entry or clears it in the write that records the outcome. `release` turns in flight into an entry for `SendAgain` and drops it for `Read` |

### P: protocol and failures

| Id | Severity | How verified | Finding | Goes to |
| --- | --- | --- | --- | --- |
| P5-1 | major | R, D | With S5-6. | Blocker, above |
| P5-2 | major | R, D | Only a signal starts the bounded wait and the supervisor. A stop from a failure, with a request on another machine that never answers, never ends. | §5.8: any stop starts them |
| P5-3 | major | R, D | A request that never answers holds a slot of the pool and its prepared change, not "nothing else". Sixteen machines waiting on a package manager's lock stop applying for the fleet. | §5.4, §5.10 |
| P5-4 | major | E, R | A read interrupted inside a frame is a lost connection. | Built; see the defects above |
| P5-5 | major | R, D | `Planned` does not say which attributes each write carries, so the engine cannot confirm attributes by a digest, nor require the secret flag on a write that carries a secret. | §5.2 |
| P5-6 | minor | E, R, D | A mode beyond the permission bits never matches. | Built; see the defects above |
| P5-7 | minor | E, D | Killing a transport does not free its standard error while a proxy it started holds the pipe; a raw pid killed after reaping can be another process. Measured: 2.8 s to end of file after killing the child, 1 µs after killing the group. | §5.8 |
| P5-8 | minor | R, E | The answer of a private process has no named shape, and nothing refuses one without a seal. | §5.3 |
| P5-9 | minor | E, R | `Look` refuses a sealed observation without a seal only on reaching it, after the observations before it ran. Measured: a file made by an earlier observation. | §5.3 |
| P5-10 | minor | R, E | `Before` as a contract violation is not limited to a `Perform` while applying, where opening today leaves a machine out; `Seen::Written` must not be a wire shape. | §5.3, §5.5 |
| P5-11 | minor | R | Outcome rows missing: a wait that ended with no answer, a shard write failing after `Done`, a premise or index out of place on a second `Perform`, a `Seen` of the wrong shape, whether a lost conversation stops the run under `stop`, and what "begins" means. | §5.5 |
| P5-12 | minor | R, D | The decoding classes contradict each other; the engine sees end of file in both, and only `ssh`'s exit code tells them apart. | §5.5, §5.12 |
| P5-13 | minor | E, R, D | Delivery's commands run before the handshake and are not retried; `proxyjump` is the whole chain, and one bastion spelt two ways is two routes; `Match exec` runs during `ssh -G`; a slot per route is fixed. | §5.3, §5.4, §5.10 |
| P5-14 | minor | R | Where one-at-a-time and control-domain changes wait is unclear, and a queue in plan order would hold unrelated changes behind one waiting. | §5.1, §5.4 |
| P5-15 | minor | R | `plan` takes the lock today, and is not named; beside a live apply it would read live marks as holds. | §5.8 |
| P5-16 | nit | R | A stop can still leave a path removed: a failed shard write, a second signal, a lost conversation. | §5.8 |
| P5-17 | nit | D | The window that merges HUP and TERM, a third signal, and a handler inherited through `fork` before `pre_exec` resets it. | §5.8 |

### S: state and invariants

| Id | Severity | How verified | Finding | Goes to |
| --- | --- | --- | --- | --- |
| S5-1 | major | E, R | A removal placed past a replacement of what its record names. | Built; see the defects above |
| S5-2 | major | E, R | A move onto a freed path planned as a creation. | Built; see the defects above |
| S5-3 | major | E, R | No edge from a replacement to the change of what was recorded inside it. | Built; see the defects above |
| S5-4 | major | D, R | A resend refused at a premise withdraws the mark, and with it an entry an earlier run left: the post-install never runs, and nothing says so. | §5.6: what the shard held before this run's mark is put back |
| S5-5 | major | D | "Resent when the reading finds the resource" still sends `dnf reinstall` of a version that is not installed, for an interrupted update or a replacement before its forget point. | §5.2, §5.6: resent only where the reading matches the declaration |
| S5-6 | blocker | D | A mark is both an entry and a hold. | Blocker, above |
| S5-7 | major | D | Group commit ties a finished change's record to the next mark on its machine, so a change waiting on another machine keeps a record another machine waits for from being written. | §5.6: a shard is written as soon as its machine has an outcome and no request ready |
| S5-8 | major | D, R | The migration leaves a moment with no `<state>`, which a version 4 binary reads as an empty state; an unfinished directory is removed under no named lock, taking `lock` with it. | §5.13 |
| S5-9 | minor | R, D | The property after an interrupted run is false for reactors, whose records gain a debt, or are made from a reading, before anything of theirs is begun. | §5.6 |
| S5-10 | minor | D | Under `continue`, each failure after operations of a `Read` change leaves what it made unrecorded. | §5.6 |
| S5-11 | minor | D | `INV-STATE-005`'s first clause asks for a mark only for kinds reading cannot confirm, while holding rests on every mark. | §5.6 |
| S5-12 | major | D | `INV-APPLY-001`, stated negatively, lets a change begin after an unknown outcome, a stop at a forget point, or between a replacement's halves. | §5.5: stated positively |
| S5-13 | minor | E, R | A held change on a machine that answered reported as out of reach. | Built; see the defects above |
| S5-14 | minor | E, R | A held control-domain change named nowhere. | Built; see the defects above |
| S5-15 | nit | E, R | A refusal naming one resource as both sides. | Built; see the defects above |
| S5-16 | minor | R, D | `waits_for` exists only as places in stages; it has to be a list on each change, and §7 misses that it and `Held` change the plan's number. | §5.5, §7 |
| S5-17 | minor | D | Ignoring names outside the hexadecimal alphabet would ignore `control`, `key`, `format` and `lock`. | §5.6 |
| S5-18 | minor | D | "Or by a person" names no command for an entry whose resend cannot succeed. | §5.8 |
| S5-19 | nit | D, R | Whether a plan with a hold has a number, and one `change` where a shard can hold several marks. | §5.8 |

### Decided by the owner after the fifth review

1. **A request that never answers holds its slot, and step 1 says so** (P5-3). One
   synchronous conversation per machine stays; reading without a slot belongs to RFC
   0003, and a budget per request stays `W-015`'s question.
2. **Under `continue`, a change that failed after its operations leaves an entry
   for removal only** (S5-10), so state loses nothing a later removal needs.
3. **Revision 5, and a full round of P and S** before acceptance.
