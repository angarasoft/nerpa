# RFC 0001. Applying in streamed batches, and an executor that replaces itself

- Status: in review; revised after the second review (§9), third review pending
- Date: 2026-09-11
- Revised: 2026-09-11, after each review
- Touches: 0006, 0010, 0011, 0015, 0020, 0026, 0027, 0028, 0031, 0032, 0033,
  0034, 0040, 0041, 0042, 0043; `INV-BATCH-001`, `INV-STATE-004`,
  `INV-STATE-005`, `INV-REACH-002`, `INV-REACT-002`, `INV-PLAN-007`,
  `INV-PLAN-008`, `INV-PREMISE-001`, `INV-PREMISE-002`, `INV-SECRET-*`,
  `INV-WIRE-001`, `INV-WIRE-003`; `W-009`, `W-014`, `W-015`, `W-022`

## 1. Summary

Applying costs one network exchange per change, one machine after another.
Reading already costs one journey per machine.

This RFC proposes the end state called D. Each machine is one conversation for
the whole run, and each stage is one batch in it. Machines run side by side. The
executor reports every operation as it completes, and the engine records each
change as it hears of it, in a state file of that machine's own. What reading
could not find again is written down before it is sent. An executor that stops
hearing from its engine stops at the next change boundary, and a run that stops
keeps its lock on the machines still working.

It gets there in three steps, and each step leaves the tool correct:

1. **Providers split into `prepare` and `conclude`, and machines apply in
   parallel, still one `Perform` per change.** State moves to one shard per
   machine, written by one recorder with group commit and a shared stop flag.
   What happens when something fails is chosen per run, and a signal stops a run
   the same way whatever sends it. The protocol does not change and nothing is
   reinstalled.
2. **The engine replaces an installed executor through that executor,** only
   under `apply`, only where the fleet allows it and the command line asks for
   it, and only by a newer release that nerpa itself verifies. **Not scheduled:**
   it is designed here so that step 3 does not rule it out.
3. **Protocol 5: streamed batches, liveness in both directions for the whole
   conversation, and stopping at change boundaries.** Delivered by step 2 if it
   exists by then, and by one more manual install if not.

Both reviews' findings, and where each is answered, are in §9.

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
  bounded, results are collected in a fixed order, and state is written from one
  place. It is built for reading only; `ARCHITECTURE.md` says "Applying does not
  yet".
- **0006:** stages order execution, so a batch never crosses a stage.
- **0015:** a second run with nothing changed has nothing to do.
- **0026:** a triggered reaction is owed, and since `a8f55b6` the debt is written
  before its cause is sent (`INV-REACT-002`).
- **0027:** a secret's value lives in memory for as long as a provider needs it.
- **0034** and **0041:** a batch carries a sealed premise, and the executor checks
  it before any operation.
- **0033:** the operation set is closed. Providers emit processes; authors never
  do.
- **0040:** a machine is spoken to only by the executor build this nerpa carries.
- **0010:** the executor is signed, and "the node checks the binary before
  replacing it". Signing is not built.
- **0042** and **0043:** a resource found as declared is recorded without being
  touched, and one whose edges changed is recorded again. So reading resolves
  most of what a run did not record: a written file is adopted and a removed one
  is already gone.
- **`INV-STATE-005`:** what a run has made is written down before it touches the
  next thing.
- **`INV-BATCH-001`:** promises one journey for reading, and says nothing about
  applying.
- **`INV-REACH-002`:** only a machine not answering lets a run continue.

**Built since the first review, and relied on below.** A change carries every
edge it comes after (`85200af`), so the plan alone can say what depends on a
change that failed. A change that sends nothing, an adoption or a reorder, is
marked so (`ChangeKind::sends_anything`). A value too large for one operation is
refused while planning (`1856bc3`), and so are two resources naming one thing on
a machine (`7aed616`). A renamed resource that keeps its path is made again after
its removal (`842abcd`). Rousing restarts only a running service (`ae25cac`), and
a provider is told whether a change was roused, whatever its kind (`4db7ffd`):
`Provider::apply` takes `roused`.

**Decided by the owner after the first review.**

- What happens when something fails is **configurable** (§5.6).
- An installed executor is replaced only after **verification on the engine**
  against a manifest nerpa carries, only with **explicit opt-in**, and never by an
  older build. Signing the manifest comes later (§5.10).
- Executor replacement and packages that carry the executor are **not
  scheduled** (`ROADMAP.md`).

**Decided by the owner after the second review.**

- **State is kept in shards, one per machine** (§5.7).
- **A run that stops keeps its lock, naming the machines still working.** The
  next run refuses only those, until nothing of that run is running there or a
  person unlocks them (§5.5).
- **A first executor replacement needs the fleet's opt-in and `--replace-executor`
  on the command line** (§5.10).
- **What a batch is about to send is written down first only where reading cannot
  confirm it afterwards,** and such a change is sent again or found by a token,
  never confirmed by reading (§5.7).

**Not decided, and what this RFC settles.**

- What state knows when a batch is interrupted.
- The provider contract for a batch.
- How an executor learns that its engine is gone, and how a person stops a run.
- What a failure does to the rest of a run.
- How an installed executor is kept current, and what checks the replacement.
- Limits on concurrency that the transports impose.

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

**One conversation per machine for the run, and one batch per stage in it.**

- A change that sends nothing, an adoption or a reorder, goes straight to the
  recorder. No batch is sent for it.
- A change whose resource is not on a machine (the control domain) runs one at a
  time, and is recorded after each.
- A change its provider marks one at a time (§5.2) is sent and recorded before its
  machine's batch for that stage is sent. A group comes before the user who needs
  it. Being one at a time is about order: it does not let a provider see an answer
  before deciding what to send.
- **A change is never split.** Chunks (§5.9), stopping (§5.3) and record points
  (§5.7) fall between changes or at points a change declares.

### 5.2 Providers: `prepare` and `conclude`

```rust
/// The kinds of change that send something. Adoptions and reorders never reach
/// a provider.
pub enum Sending { Create, Update, Replace, Delete, React }

/// Everything a provider is told to build one change.
pub struct Prepared<'a> {
    pub address: &'a Address,
    pub kind: Sending,
    /// Something it reacts to changed, or an earlier run left the reaction
    /// unpaid. Built: `Provider::apply` takes it since `4db7ffd`.
    pub roused: bool,
    /// Settled values, secrets included; empty for a removal.
    pub desired: &'a Attributes,
    /// What state recorded, for a replacement and a removal.
    pub recorded: Option<&'a Attributes>,
    pub facts: Facts<'a>,
}

/// What `conclude` is told: the same change, with no secret value in it.
pub struct Concluding<'a> {
    pub address: &'a Address,
    pub kind: Sending,
    pub facts: Facts<'a>,
    /// Settled values that are not secrets, and the names of those that were.
    pub plain: &'a Attributes,
    pub secret: &'a BTreeSet<String>,
}

trait Provider: Send + Sync {
    fn read(&self, batch: &[Reading], facts: Facts<'_>, reach: &dyn Reach)
        -> Result<Read, ProviderError>;                        // unchanged

    /// The operations that make this change, and where it may be recorded
    /// part way. No machine.
    fn prepare(&self, change: &Prepared<'_>) -> Result<Planned, ProviderError>;

    /// What the change became at a point, from exactly the answers to its own
    /// operations up to that point.
    fn conclude(&self, change: &Concluding<'_>, point: Point, became: &[Seen])
        -> Result<Concluded, ProviderError>;

    /// What the next run does with this change if a run sent it and never
    /// recorded it.
    fn afterwards(&self, address: &Address, kind: Sending) -> Afterwards;

    /// Whether this change is sent and recorded on its own.
    fn one_at_a_time(&self, change: &Prepared<'_>) -> bool { false }

    fn fake() -> Self where Self: Sized;
}

pub struct Planned {
    pub operations: Vec<Operation>,
    /// Where the change may be recorded before its last operation.
    pub points: Vec<Point>,
}

pub struct Point { pub after: usize, pub records: Records }
pub enum Records { Forget, Partial }
pub enum Concluded { Became(Attributes), Gone }

pub enum Afterwards {
    /// Reading finds it: adopted, reordered, updated or made again as usual.
    Read,
    /// Reading cannot tell: sent again.
    SendAgain,
    /// Its identity is minted over there: found by the token it was sent with,
    /// or refused until a person decides.
    FindByToken,
}
```

The rules:

- **A provider never reaches a machine outside `read`.** Neither half is given a
  `Reach`.
- **One change's operations are contiguous.** `conclude` is handed exactly its
  own answers, up to the point it is asked about.
- **A replacement is one `prepare`:** delete operations built from `recorded`,
  create operations from `desired`, and a `Forget` point between them.
- **A removal is given what was recorded and nothing desired, and the provider
  decides what removal means for its kind.** A file is removed; a service is
  stopped and disabled rather than restored to what was recorded.
- **`conclude` never sees a secret value.** A secret lives from `prepare` until
  its batch is written to the wire, inside the operation that carries it (0027).
- **A refusal from `conclude` follows operations that ran.** It never means
  nothing was done (§5.6, §5.13).
- **No branching inside a batch.** A provider that needs to decide on an
  intermediate result splits the work across stages (`W-009`). Being one at a
  time does not help: `prepare` returns every operation before any is sent.
- **Operations are idempotent, or observed.** Tested for files and directories
  with an in-memory executor. Packages and services need a stateful fake,
  specified before coverage is claimed (§5.12).
- **`afterwards` is `Read` only where reading sees everything the change does.**
  Files, directories and a service's settings are `Read`. A package's install or
  removal is `SendAgain`, because a package is registered before its post-install
  runs. A resource whose identity a machine or a cloud mints is `FindByToken`.
- **Every `ProviderError` is governed by the failure policy.** What always stops a
  run is internal to the engine (§5.6).
- **Facts are decoded in one helper,** twice per change, once for each half.
  Accepted.

**A stage is prepared whole before any of it is sent.** Secrets are fetched here.

The trait changes now because only one provider implements it: the `os`
provider, plus the test fakes. That is the argument 0031 made for `Send + Sync`.

### 5.3 Protocol 5

**Opening.** The transport is spawned, the far end is asked a bootstrap `Build`,
and only then does any stamped frame go out, `Alive` included. A handshake is
retried only before the first `Perform`.

**A bootstrap that cannot be mistaken for anything else.** A CBOR map whose only
key is `nerpa-bootstrap-1`, decoded with unknown fields refused at every level.
Stamped frames are decoded the same way, and neither decoder is ever tried on a
frame shaped for the other. `INV-WIRE-001` becomes: a frame is the bootstrap,
recognised by its envelope and decoded exactly, or it is stamped; nothing is read
as a subset. What the bootstrap freezes, for ever: `Build` and its answer, and the
operations replacement needs (§5.10) — walking a path without following links and
reporting owner, mode and label; creating a file exclusively and syncing it;
running a file with input under a timeout; linking and renaming without following
links; syncing a directory. Byte-level vectors for each live in `tests/wire.rs`.

**Every request carries an id:** `Perform`, `Look` and `Build` alike.

| Frame | Direction | When | Final |
| --- | --- | --- | --- |
| `Answer::Progress { seq, at, became }` | executor → engine | after operation `at` | no |
| `Answer::Working { seq, at }` | executor → engine | when the writer has sent nothing for `H` seconds | no |
| `Answer::Finished { seq }` | executor → engine | after the last operation | yes |
| `Answer::Failed { seq, at, problem }` | executor → engine | a premise could not be read, or an operation failed; `at` says which | yes |
| `Answer::Moved { seq, premise }` | executor → engine | a premise did not hold; nothing was done | yes |
| `Answer::Stopped { seq, before_change, reason }` | executor → engine | stopped at a change boundary: asked, input ended, or the engine went quiet | yes |
| `Answer::Refused { seq, problem }` | executor → engine | refused before anything ran: no seal, a malformed or reused `seq`, a second `Perform` while one runs | yes |
| `Ask::Alive` | engine → executor | every `E` seconds for the whole conversation, after `Build` | — |
| `Ask::Stop { seq }` | engine → executor | stop at the next change boundary; ignored for any other `seq` | — |

**One state machine for the stream,** in `nerpa-proto`, shared by the executor's
writer and the engine's validator:

- `Progress.at` is exactly the next operation, and `Finished` comes only after the
  last;
- `Failed` names the next expected operation, or a premise;
- `Stopped.before_change` equals the number of changes fully reported;
- a `Working` may name the operation just reported, because the heartbeat and the
  answer race;
- at most one final frame per request, and nothing after it.

A frame that decodes and breaks one of these rules is a protocol error, and ends
the conversation (§5.6).

**An executor that has stopped stays stopped.** Once input has ended, or nothing
has arrived for `L` seconds, the conversation is over: a later frame is answered
with `Stopped` if the pipe still carries, and the executor exits. The checks run
after the premise is checked as well as before each change, so a `Perform`
delivered after the silence runs nothing.

**The engine times the executor too.** No frame for `L_e` seconds, several times
`H`, is a conversation lost.

**The transport's standard error** is drained by a thread of its own into a
bounded buffer, and kept for the report.

A version 4 executor refuses a version 5 frame by name and exits, which is
measured. §5.13 has a row for meeting one.

### 5.4 The executor: three threads

- **Reader.** Frames from stdin. It handles `Alive` and `Stop` itself, updating a
  time and a flag, and forwards only requests. The engine never sends a second
  request before a final answer, so that queue holds one.
- **Worker.** Carries out one `Perform`. It checks the stop flag, the end of input
  and the deadline after the premise and before each change.
- **Writer.** The only thing that writes stdout, and the one that emits
  `Working`, so the order of frames holds by construction.

**The executor exits when any of its threads ends.** A panic in one aborts the
process, so a half-dead executor cannot keep a conversation alive. The rule
against aborting exists for nerpa's lock guard, which the executor does not hold.

`Working` proves that the process is alive and the operation has not returned.
It does not prove the operation is getting anywhere: a package manager waiting on
a lock nobody will release still holds a run. `W-015` is not narrowed; a budget
per operation is a separate decision.

### 5.5 The engine: running a stage

1. **Prepare the whole stage** (§5.2).
2. **Group by machine.** Adoptions and reorders go to the recorder. Control-domain
   and one-at-a-time changes go first, one at a time.
3. **Write ahead** what reading cannot confirm (§5.7), once per machine per stage,
   through that machine's group commit, and owe every reaction a change will rouse
   (`INV-REACT-002`, built). A machine waits for its own write-ahead, never for a
   record.
4. **Send the batches.** A worker takes a slot for the machine's route before it
   commits to the machine, so a saturated jump host never holds workers another
   route could use (§5.11). Each conversation has a writer, a reader and a drainer
   for standard error. `Reach::ask` stays, as a synchronous request carried over
   them. The ask is borrowed when sealing, the answer body is consumed rather than
   copied, and a digest is computed as a stream.
5. **Readers forward** `(machine, change, point, became)` to the recorder. A
   machine sends its next batch only after its previous one is recorded, which
   bounds the recorder's backlog at one batch per machine.
6. **The recorder is the only writer of state (0031).** It calls `conclude`,
   records, and writes each machine's shard with group commit (§5.7).
7. **On a failure,** the policy decides (§5.6).
8. **Stopping.** SIGINT, SIGTERM and SIGHUP are handled alike. The first sets the
   stop flag and sends `Stop` to every conversation; each batch ends at a change
   boundary; the recorder writes what arrives; after a bounded wait the run
   reports and exits with a failure. The second kills the transports.
   **Either way, the lock is kept while any conversation has a request with no
   final answer.** It is rewritten to name each such machine, request and change,
   and when `Alive` was last sent. The next run refuses those machines, not the
   whole run, until opening shows that nothing of that run is still running there,
   or a person unlocks them. SIGTSTP is blocked while applying.
9. **Transports run in a session of their own** (`setsid`), so the terminal's
   signals reach nerpa and not `ssh`, and a password or re-login prompt fails at
   once instead of waiting. Closing a conversation waits a bounded time, then
   kills. Delivering an executor works the same way.

nerpa has no signal handling today. The process dies on the first Ctrl-C and
leaves its lock behind, which is the safe outcome by accident.

### 5.6 When something fails

Two policies, chosen per run. The first review asked for one of them (S-7); the
owner decided both are needed.

**`stop`, the default.** The first failure anywhere sets the stop flag. No
`Perform` begins to be written after that, batches in flight are sent `Stop` and
end at a change boundary, every failure is reported and the run exits with a
failure. This is today's behaviour, made parallel.

**`continue`.** A change that failed is refused, and so is everything that comes
after it in the plan, transitively. The plan carries those edges: a declared
change's `depends_on`, and for a removal, the removals that wait for it, taken
from the records that name it. The rest of a batch after `Failed` is sent again in
the same stage without premises, which were checked once already. `Moved` refuses
only the changes that own the premise that moved, and the rest are sent again the
same way. Everything else goes on, on that machine and on others. The run ends
with every failure and every refusal named, with its cause, and exits with a
failure.

**What each outcome counts as.**

| Outcome | Counts as | The conversation | Pending entries of changes never sent |
| --- | --- | --- | --- |
| `Failed` | a failure of that change | kept | withdrawn |
| `Moved` | a failure of the changes owning the premise | kept | withdrawn |
| `Refused` for a missing seal or a malformed frame | a contract violation | ended | withdrawn |
| `Refused` for anything else | a failure of the batch | kept | withdrawn |
| `Stopped`, asked for | the stop in progress | ended | withdrawn |
| `Stopped`, not asked for | a conversation lost | ended | withdrawn |
| A frame that decodes and breaks the stream's rules | a contract violation | ended | kept for the request in flight |
| A frame that does not decode, or the transport exits | a conversation lost | ended | kept for the request in flight |
| No frame for `L_e` | a conversation lost | ended | kept for the request in flight |
| `conclude` refuses | a failure, after its operations ran | kept | kept for that change |
| A failure to prepare | `stop`: nothing of the stage is sent; `continue`: that change and its dependents are refused | — | none written |

A lost conversation refuses that machine's remaining changes, and their
dependents elsewhere. It is reported as a machine lost part way, never as one that
did not answer to begin with (`INV-REACH-002`).

- **Always a stop, whatever the policy:** the state write fails, a contract is
  violated, or a signal arrives.
- **Reactions.** A reacting resource comes after its cause, so it is refused when
  its cause fails. Its debt was written before the cause was sent, so the next run
  still rouses it: at most one redundant restart, the window 0026 accepts.
- **Where it is set, and where it shows:** `[apply] on_failure = "stop"` or
  `"continue"` in the fleet document, overridden by `nerpa apply --on-failure`,
  and printed in the plan's header beside every machine allowed to replace its
  executor. Not per group: a policy per group has to say what stopping one group
  means for another in the same stage, and nothing needs that yet.

**Invariants.** `INV-REACH-002` keeps its wording and is scoped to planning. A new
invariant, `INV-APPLY-001`: **no `Perform` carrying a change begins to be written
after a change it comes after has failed,** over the edges the plan carries for
each kind; and under `stop`, **none begins after the stop flag is set.** Restoring
an executor after a failed replacement is a named exception (§5.10). The
postcondition in `apply` becomes: changed, failed and refused add up to the plan,
and `Applied` carries each failure and refusal with its cause.

**Risk, accepted:** `continue` leaves a fleet part changed on purpose. It is
named in every refusal, in the plan's header and in the exit status, and it is not
the default.

### 5.7 State: shards, writing ahead, recording

**Shards.** State is a directory: one JSON file per machine, one for the control
domain, and the lock. A write rewrites only its machine's shard, so it costs that
machine's state rather than the fleet's, and machines write in parallel. For one
file on this control machine: 13 ms at 100 resources, 20 ms at 1000, 60 ms at
10 000 and 538 ms at 100 000, with loading at 842 ms; below 1000 resources the
two `fsync`s dominate. Each shard is written whole or not at all
(`INV-STATE-004`, per shard), so after a crash shards may describe different
moments, and every record in them was true when it was written. Edges across
machines are addresses and need nothing new. Loading reads every shard once per
run.

**Writing ahead only where reading cannot confirm.** Before a batch is sent, the
recorder durably writes a pending entry for each change in it whose provider says
`SendAgain` or `FindByToken` (§5.2). Files, directories and a service's settings
get no entry. Reading finds what an interrupted run did to them — adopted (0042),
reordered (0043), updated or made again — and a lost record of an update leaves
attributes whose only use is to find the resource. A restart is covered by its
debt (`INV-REACT-002`).

**What an entry holds:** the address, the kind, the locator it was sent with, its
edges, the time, the record point passed, and for minted identity the client
token it was sent with.

**Resolving an entry.** It is never confirmed by reading.

- **Sent again.** The next plan proposes the change again and says when it was
  sent and not confirmed. That time is kept outside the plan's number, as a
  premise is, and never shown as an age. A removal is sent again the same way,
  since already being gone is not a failure.
- **Found by token.** The provider looks for the resource by the token. Found, it
  is recorded. Not found, the change is refused until a person imports or forgets
  it; it is never made a second time on its own.

Because no entry waits on a confirmation, a resource that reads as declared and
is recorded has nothing to clear, and the second run after an interrupted one is
empty (0015).

**Clearing.** An entry is cleared by the record of what its change became, or by
the write that records a stop or a refusal for a change never sent.

**Group commit, per shard.** While one durable write of a shard is in progress,
completions and write-ahead entries for that machine queue up, and the next write
carries all of them.

**Record points** (§5.2) are `Forget` or `Partial`. A change's pending entry
survives every point until its final record, and says which point was passed.

**In step 1.** Machines in parallel already need one writer and a shared stop, so
the recorder, the stop flag, group commit, shards and pending entries arrive with
step 1.

**The format** goes to version 5, a directory. A version 4 file is read once and
split. The migration is one-way: an older nerpa cannot read the result, and a
downgrade command refuses while any entry is pending.

**Invariants.** `INV-STATE-005` splits in two:

- "Nothing that reading cannot find again is sent before a durable write names it
  pending."
- "A pending entry is cleared only by a record of what its change became, or by a
  write saying it was never sent."

How soon a completion reaches disk, the next write of its shard, is a stated
bound rather than an invariant.

**The property.** A completed run leaves the same state whatever order machines
finish in, `written` aside (0031's fixed order). After an interrupted run, each
address holds the record from before the run, one written at a declared point, or
the completed run's. An address that does not hold the completed run's record has
a pending entry, or is one reading resolves. Debts are a superset of the
completed run's, and each extra one names a cause that was pending or never sent.

### 5.8 Premise

**Checked once per batch, before its first operation,** over the union of its
changes' premises, sealed for the machine (0034, 0041).

A later change whose premise an earlier change in the same batch moved has been
moved by this run, not by anyone else. Installing a package that pulls in another
changes the second one's reading. Two resources naming one thing are refused
while planning (`INV-PLAN-008`), and `conclude` reads back what each change
became. The replacement path already reasons this way: the create half is not
checked against a premise its own delete just moved.

**Absence is a premise too.** A create carries the observation that found
nothing, so a thing that appears between reading and applying refuses the batch
instead of being written over. Today a create carries no premise at all.

**Resent remainders** under `continue` carry no premise (§5.6).

**Step 1** still sends one `Perform` per change, each with its own premise. P-6
was not reproduced, and it is narrower than first stated: it needs a second
package planned as an update whose version the first one's install changes.
Whether step 1 checks a machine's premises once per stage waits for a
reproduction.

**Risk, bounded:** a change made by somebody else between the check and a later
operation of the same batch, or of its resends, is not detected. Neither are two
resources reaching one file through a symbolic link or a reference, which is
`INV-PLAN-008`'s bypass. The next read sees both.

### 5.9 Size

A batch is split into chunks by encoded size (8 MiB) and by operation count
(1000), **only between changes**, well under `MAX_FRAME`. Chunks run in order in
one conversation, and the premise is checked before the first.

**A change's own encoded size is bounded while planning:** the total of its
operations, where `INV-PLAN-007` bounds one attribute today. A change too large
for a frame is refused before anything is sent; one larger than a chunk is sent as
a chunk of its own, up to that bound.

### 5.10 Replacing an installed executor (step 2, not scheduled)

**What nerpa carries.** A manifest compiled into it: the full digest,
architecture and protocol of the executor beside it; a build number that only
rises from release to release; and the manifests of the earlier releases it
knows. A development build has build number 0 and replaces nothing.

**When it happens.** All of these, or the machine is refused as 0040 refuses it
today, naming both builds and the rule that refused:

- the task is `apply`, and the command line says `--replace-executor`;
- the fleet allows it for that machine: `replace_executor = true` on the node or
  one of its groups;
- the installed executor answers `Build` with a full digest this nerpa's list
  names, at a lower build number;
- the file beside nerpa has the manifest's digest, and the machine's `arch` is
  the manifest's.

The plan's header lists every machine whose executor it will replace. The digest
is what the far end reports about itself, so the trigger is its choice, bounded to
installing a genuine build.

**Before reading, on its own.** A machine whose executor is not recognised is not
read through it. Its plan holds the replacement, and every declared resource on
it listed as not looked at, with the reason. Its own changes wait for the next
run, which reads it with the new executor.

**The sequence.**

1. **Walk where it goes.** The executor walks `P`'s path from the root, one
   component at a time without following links, and reports owner, mode and label
   for each. `P` must be a regular file, every directory owned by root and
   writable by nobody else, and no component a link. That report, with `P`'s
   content digest, is the premise of everything after it.
2. **Beside, not over.** Create `.nerpa-executor-<digest>-<random>.new`
   exclusively in the walked directory, mode 0755, and sync it.
3. **Health check, with a timeout.** Run the new file with a bootstrap `Build`. It
   must answer the carried digest, and its label must match `P`'s.
4. **Keep the previous, then switch.** The executor links `P` to `P.previous` and
   renames the new file over `P`, relative to the walked directory and without
   following links, then syncs the directory.
5. **Probe through the transport.** A fresh conversation must open and answer
   `Build` with the new digest before the replacement counts. If it does not,
   `P.previous` is restored through the old conversation, which keeps sending
   `Alive` throughout. If that conversation is gone as well, the refusal says so,
   and the next run finds the new build at `P`.

One `P.previous` is kept. The next replacement removes `.new` files a failed one
left behind.

**What the check proves:** the new binary starts, both under the old executor and
through the transport, speaks the bootstrap, and is the build nerpa carries. Not
that every operation works on that machine.

**Risk, accepted.** Replacement runs as root, and until signing lands the
manifest is only as trustworthy as the nerpa binary holding it. A check on the
machine would protect nothing a compromised engine cannot route around, which is
why the check is on the engine. **The other way round holds too:** a compromised
executor performs and reports every step of its own replacement and can fake all
of them, so replacement is no way to recover a suspect executor. That stays a
reinstall by root.

This refines 0040: another build is replaced rather than refused, when every rule
above holds.

### 5.11 Limits the transports impose

| Limit | Value | Consequence | Response |
| --- | --- | --- | --- |
| OpenSSH `MaxStartups` on a shared bastion | `10:30:100` by default | More than 10 unauthenticated connections start being dropped at random. Today's fan-out of 16 through one jump host can exceed it, when reading as well | A slot per first hop, default 8, taken before a worker commits to a machine, inside the global 16; both set in the fleet's `[apply]` table. Retry pre-authentication drops with jittered backoff. Recommend `ControlMaster` on the jump hop. A simulated fleet measures the defaults |
| OpenSSH `MaxSessions` | 10 per connection | Counts shell, login and subsystem sessions; forwarding channels are not named | Verify before relying on multiplexing through a jump host (§8) |
| Teleport role `max_sessions` | 10 per SSH connection | nerpa opens one session per connection | None needed |
| Teleport proxy `connection_limits.max_connections` | 1000 per client IP, per service | Far above this fleet | Name it in the operator guide |
| One `ssh` process and three threads per conversation | `W-014` | Memory and process count on the control machine | Unchanged; the pool bound is per route |
| Durable state writes | 2 `fsync` per write; a write costs its shard | Linear in the number of writes | Group commit per shard (§5.7) |
| Memory | the largest change per machine in flight | Up to `MAX_CONTENT`, 32 MiB, so 512 MiB for 16 machines at worst | The bound on a change's size (§5.9) is the lever |
| The transport's standard error | unbounded | A verbose route fills the pipe and stalls `ssh` | Drained into a bounded buffer |

### 5.12 Testing

This is new policy for `docs/TESTING.md`: the suite has property and adversarial
tests, and no fault injection.

- **An in-memory executor that interprets operations.** Files and directories
  get replay tests: a batch applied twice converges. Packages and services need a
  stateful fake registry per family and init system, specified before their
  coverage is claimed.
- **A clock the tests control,** for every liveness rule. The few tests on real
  timers run in a slower layer, like the live harness.
- **The stream's state machine,** tested from a table of every transition and
  every rejected frame; a bootstrap frame with one extra field refused, and a
  stamped frame carrying bootstrap fields refused.
- **The real executor behind a pipe:** input closed mid-batch; silence past `L`
  followed by a `Perform`; a `Perform` followed at once by the end of input; a
  `Stop` crossing a `Finished`; each thread made to panic in turn.
- **A recorder property:** the one stated in §5.7, for completed and interrupted
  runs.
- **A failure-policy property:** under `continue`, nothing after a refused change
  is sent, over declared edges and removal edges alike; under `stop`, no `Perform`
  begins after the flag.
- **Replacement:** refuses another architecture, an older or unknown build, a
  development build, a machine without the fleet's opt-in, a run without the flag,
  a link anywhere on the path, and `plan`; the switch is a rename; restoring works;
  the label check runs on Rocky.
- **A gate like `xtask budget`** for recorder throughput per shard under
  concurrent completions, and for the simulated fleet's defaults. Step 1 is
  measured on its own, before and after.
- **The live harness** used for §2.3, on all three routes, as part of the exit
  check for step 3.

### 5.13 Failure analysis

| Event | Machine | Engine knows | State says | Next run |
| --- | --- | --- | --- | --- |
| A premise moved | Nothing of that batch done | `Moved { seq, premise }` | Entries for the batch withdrawn | `stop`: the run stops. `continue`: the changes owning the premise and their dependents are refused, and the rest is sent again |
| An operation fails | Operations before it done | `Progress` up to it, `Failed` | Up to the last reported change; the failed change's entry kept, if it has one | `stop`: the run stops. `continue`: its dependents are refused, and the rest of the batch is sent again. The next run proposes it again |
| `conclude` refuses | The change's operations ran | A refusal after action | Its entry kept, if it has one | A failure under either policy; the next run reads what it actually is |
| A signal, once | Stops at a change boundary, or not within the wait | `Stopped` from each machine that made it | Everything reported; the lock kept for machines with no final answer | Continues where it stopped; those machines refused until nothing of that run is running there, or a person unlocks them |
| A second signal | Transports killed; executors stop when input ends or `Alive` is missed | What was in flight, printed | The last write; the lock kept, naming what was in flight | As above |
| Engine killed | Stops at the next change boundary once input ends (step 3); completes the batch today | Up to the last `Progress` | Up to the last recorded change; pending entries; the dead process's lock | Refused by the lock until a person unlocks. Then files and directories are resolved by reading, and pending entries are sent again or found by token |
| Engine gone, Teleport route | Input does not end for about a minute under `sudo -n` (measured 59 s); stops when `Alive` is missed for `L` | Same | Same | Same |
| Network partition | Stops when `Alive` is missed for `L`; `ServerAlive*` gives up after 90–120 s | A conversation lost, or an unasked `Stopped` if the link heals in between | Up to the last recorded change | The policy decides the rest of this run; the machine is reported lost part way |
| The executor crashes | The operation in flight unknown; a process it started, such as a package manager holding its lock, may live on | A conversation lost | Same | Reads the world; the package manager's lock may refuse the next install until it is released |
| An executor thread dies | The executor aborts | A conversation lost | Same | As a crash |
| A stream protocol error | The executor may still be working | The conversation ended, as a contract violation | Same | Always a stop |
| The machine reboots mid-batch | Same, and a write the executor never synced may be lost | Same | May record a write the machine lost | The next read sees the difference and proposes it again. An owed reaction is still owed (0026) |
| A version 4 executor meets a version 5 engine | Exits on the first stamped version 5 frame | Refused as another build first, by `Build` | Unchanged | Replaced if step 2 and both opt-ins allow it; otherwise refused with manual instructions |
| Writing ahead fails | Nothing of that machine's batch sent | A failure to write state | The last successful write | Always a stop |
| The state write fails (disk full) | Batches in flight get `Stop` | Everything reported before the failure | The last successful write | Always a stop. A person fixes the disk; the next run reads the world |
| `prepare` fails, a secret included | `stop`: nothing of that stage sent. `continue`: the rest of it sent | A refusal before action | Unchanged | As today |
| The replacement fails its health check | Old executor untouched, new file removed | Refused, naming both builds | Unchanged | Refused until fixed |
| The replacement passes, but the probe through the transport fails | `P.previous` restored through the old conversation | Refused | Unchanged | Refused, naming what failed |
| The probe fails and the old conversation is gone | The new build is at `P` | Refused, saying the restore could not be sent | Unchanged | The next run finds the new build at `P` |
| Two engines on one machine at once | Two conversations | Each knows its own | Each lock protects its own state | Premises catch overlapping writes where there are premises; no lock across engines (§8) |

### 5.14 Migration

| Step | Changes | Protocol | Reinstall | Invariants |
| --- | --- | --- | --- | --- |
| 1 | `prepare`/`conclude` for every kind that sends, with what state recorded and without secrets in `conclude`; `afterwards` per kind; machines in parallel with a `Perform` per change; state in shards (format 5), the recorder, stop flag, group commit and pending entries where reading cannot confirm; the failure policy; the lock kept for machines in flight; signals and `setsid`; route slots; absence as a premise; a bound on a change's size; handshake retry before the first `Perform` | 4 | No | `INV-STATE-005` split (§5.7); `INV-STATE-004` per shard; `INV-BATCH-001` extended to name applying; `INV-REACH-002` scoped to planning; `INV-APPLY-001` (§5.6) |
| 2, not scheduled | Replacement through the installed executor, verified on the engine, with the fleet's opt-in and a flag; the bootstrap, with the walking and linking operations it needs | 4, and the bootstrap | No | 0040 refined; `INV-WIRE-001` reworded (§5.3) |
| 3 | Streamed batches with an id on every request and one state machine for the stream; `Refused`; `Alive` after `Build` for the whole conversation; `Stop` at change boundaries; the three-thread executor that exits when a thread ends; the premise once per batch; a digest rather than the content when reading back a large file | 5 | By step 2 if it exists, otherwise by hand | `W-015` unchanged (§5.4) |

## 6. Alternatives

- **A only: parallel machines, a `Perform` per change, for good.** Simplest. It
  leaves 16–22× on the table (§2.1).
- **C: batches without streaming.** No protocol change, but a lost connection
  loses a whole batch's worth of knowledge, and it needs `INV-STATE-005` weakened
  to "per batch".
- **D now, installed by hand.** Same end state, one more manual install. Every
  later protocol change costs another until step 2 exists.
- **A resident agent,** a daemon on each machine. It would make liveness and
  progress easy, and it is exactly what 0010 and 0032 chose not to be.
- **Recovery by reading alone.** Chosen for what reading can confirm (§5.7). Not
  for packages, which are registered before their post-install runs, or for
  minted identity, which a declaration cannot find.
- **Confirming a pending entry by reading, as a kind of change of its own, or
  treating the address as unrecorded so that it is adopted.** Not chosen: both
  still need a rule per kind for packages and minted identity; the first adds a
  kind of change and a plan that is not empty after every interruption; the second
  tells a reader that a resource nerpa already owned is adopted.
- **State in one file with a stated limit,** or **a log with compaction.** Not
  chosen: the first stops helping at the fleet size 0031 aims at, and the second
  is harder to read by hand and to recover from a torn tail.
- **Releasing the lock after waiting `L`,** or **keeping the whole lock until a
  person unlocks.** Not chosen: the first makes a second signal wait a minute and
  leans on each route's lingering, and the second lets one stuck machine stop a
  fleet.
- **Replacement enabled by the fleet alone.** Not chosen: an edit to the fleet by
  one person would enable a root replacement for whoever runs `apply` next.
- **One failure policy for everyone.** Not chosen, by the owner's decision.
- **Checking a replacement on the machine,** as 0010 words it. Not chosen now:
  the first replacement cannot, and a check a compromised engine can route around
  protects nothing.

## 7. What this costs

- **The provider trait changes,** and every provider says per kind whether reading
  confirms a change. Today that means the `os` provider and the test fakes; later
  it is paid by every provider author, which is why it happens now.
- **The executor becomes threaded,** and so do the ways it can deadlock.
- **The engine gains signal handling, a recorder, and three threads per
  conversation.**
- **State becomes a directory of shards,** format 5, and the migration is
  one-way.
- **A package interrupted mid-install is installed again,** whether or not the
  interrupted install finished.
- **A run that stops uncleanly leaves machines locked** until opening shows
  nothing is running there or a person unlocks them.
- **The wire grows** by seven frames, an id on every request, and a bootstrap
  whose encoding and operations can never change.
- **A failure policy is a setting,** and one of its values leaves a fleet part
  changed on purpose.
- **A replacement runs as root, unsigned** until signing lands, and needs a flag
  as well as the fleet's opt-in. Every release carries the manifests of the ones
  before it, and a development build never replaces anything.
- **A person reading a plan sees new lines:** replacing the executor, the policy
  and replacements in the header, and changes sent again after an interruption.

## 8. Open questions

1. **Teleport lingering: answered by measurement on 2026-09-11.** With a probe every
   second and the local proxy tracked, the proxy exited within a second of `ssh`,
   and the executor lived about 59 s longer. The cause is the route. `Alive` is
   required, and `L` must be well under that to matter on Teleport.
2. **Values for `E`, `L`, `H` and `L_e`,** and how they relate: `E` well under
   `L`, `H` well under `L_e`, and `L` against `ServerAliveInterval` times
   `ServerAliveCountMax`.
3. **Multiplexing through a jump host.** Is `MaxSessions` counted for forwarding
   channels?
4. **Signing the manifest:** where the key lives, how nerpa verifies it, and how a
   key is revoked.
5. **How opening tells that nothing of an earlier run is still running on a
   machine** (§5.5): a marker the executor holds for as long as a `Perform` runs,
   or a look at the process table.
6. **When the executor syncs what it writes:** per operation, per change or per
   batch, measured on the targets.
7. **P-6 as an update,** if it is to be reproduced before step 1 decides how often
   to check premises.
8. **Rolling out a change to part of a stage first** (`serial`, canaries). Out of
   scope; it deserves its own RFC.
9. **A failure policy per group.** Not proposed; revisited when a fleet needs it.
10. **A journal of each run beside state,** saying what it sent and never
    confirmed, for a person to read. Not state, and not part of any plan.

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
| S2-10 | R, E | Rousing sent a plain restart, which starts a stopped service, while the comments and the architecture said it stays down. | `ae25cac`, `INV-REACT-003` |
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
