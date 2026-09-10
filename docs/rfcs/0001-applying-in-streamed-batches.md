# RFC 0001. Applying in streamed batches, and an executor that replaces itself

- Status: in review
- Date: 2026-09-11
- Touches: 0006, 0010, 0011, 0020, 0026, 0031, 0032, 0033, 0034, 0040, 0041;
  `INV-BATCH-001`, `INV-STATE-005`, `INV-PREMISE-001`, `INV-PREMISE-002`,
  `INV-WIRE-001`, `INV-WIRE-003`; `W-009`, `W-014`, `W-015`, `W-022`

## 1. Summary

Applying costs one network exchange per change, one machine after another.
Reading already costs one journey per machine.

This RFC proposes the end state called D. Each stage of a plan is one
conversation per machine, and machines run side by side. The executor reports
every operation as it completes. The engine records each change as it hears of
it. An executor that stops hearing from its engine stops between operations.

It gets there in three steps, and each step leaves the tool correct:

1. **Providers split into `prepare` and `conclude`, and machines apply in
   parallel, still one `Perform` per change.** The protocol does not change and
   nothing is reinstalled. `INV-STATE-005` is untouched.
2. **The engine replaces an installed executor of another build through that
   executor.** The protocol does not change. This refines 0040, which refuses
   such a machine today.
3. **Protocol 5: streamed batches, liveness in both directions, and stopping.**
   Step 2 delivers it, so the manual install of 2026-09-11 is the last one.

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
| Teleport, `sudo -n` | completed: a, b, c | `sudo` and the executor alive at +13 s, gone by +69 s; the same when the local `ssh` got SIGTERM instead of SIGKILL |
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
- **0026:** a triggered reaction is recorded as owed before it is attempted.
- **0034** and **0041:** a batch carries a sealed premise, and the executor checks
  all of it before any operation.
- **0033:** the operation set is closed. Providers emit processes; authors never
  do.
- **0040:** a machine is spoken to only by the executor build this nerpa carries.
- **0010:** the executor is signed, and "the node checks the binary before
  replacing it". Signing is not built.
- **`INV-STATE-005`:** what a run has made is written down before it touches the
  next thing.
- **`INV-BATCH-001`:** promises one journey for reading, and says nothing about
  applying.

**Not decided, and what this RFC settles.**

- What state knows when a batch is interrupted.
- The provider contract for a batch.
- How an executor learns that its engine is gone, and how a person stops a run.
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

**One conversation per stage per machine.**

- A change whose resource is not on a machine (the control domain) runs one at a
  time, as today, and is recorded after each.
- So does a change whose provider asks for it (§5.2).

### 5.2 Providers: `prepare` and `conclude`

```rust
trait Provider: Send + Sync {
    fn read(&self, batch: &[Reading], facts: Facts<'_>, reach: &dyn Reach)
        -> Result<Read, ProviderError>;                        // unchanged

    /// The operations that make this change, and nothing else. No machine.
    fn prepare(&self, change: &Prepared<'_>) -> Result<Vec<Operation>, ProviderError>;

    /// What the change became, from what exactly its operations answered.
    fn conclude(&self, change: &Prepared<'_>, became: &[Seen])
        -> Result<Attributes, ProviderError>;

    /// The operations that remove what `recorded` describes.
    fn prepare_removal(&self, address: &Address, recorded: &Attributes, facts: Facts<'_>)
        -> Result<Vec<Operation>, ProviderError>;

    /// Changes this provider makes that must be recorded one at a time: a
    /// resource whose identity the machine mints when it is created.
    fn one_at_a_time(&self, address: &Address) -> bool { false }

    fn fake() -> Self where Self: Sized;
}
```

The rules:

- **A provider never reaches a machine outside `read`.** The signatures enforce
  it: neither `prepare` nor `conclude` is given a `Reach`.
- **One change's operations are contiguous.** `conclude` is handed exactly its
  own slice of the answer.
- **No branching inside a batch.** A provider that needs to decide on an
  intermediate result splits the work across stages (`W-009`), or asks for
  `one_at_a_time`.
- **Operations are idempotent, or observed.** Re-running a batch whose outcome was
  lost must converge: a file rewritten, a package already present, a service
  already running. A provider emitting a process that is neither says
  `one_at_a_time`.

The trait changes now because only one provider implements it: the `os`
provider, plus the test fakes. That is the argument 0031 made for `Send + Sync`.

### 5.3 Protocol 5

`Ask::Perform` keeps its shape. Its answer becomes a stream:

| Frame | Direction | When | Final |
| --- | --- | --- | --- |
| `Answer::Progress { at, became }` | executor → engine | after operation `at` | no |
| `Answer::Working { at }` | executor → engine | every `H` seconds while operation `at` runs | no |
| `Answer::Finished` | executor → engine | after the last operation | yes |
| `Answer::Failed { at, problem }` | executor → engine | an operation failed | yes |
| `Answer::Moved { at }` | executor → engine | a premise did not hold; nothing was done | yes |
| `Answer::Stopped { at }` | executor → engine | stopped before operation `at` | yes |
| `Ask::Alive` | engine → executor | every `E` seconds while a `Perform` runs | — |
| `Ask::Stop` | engine → executor | a person or a failure elsewhere stops the run | — |

**The executor stops between operations** when a `Stop` arrives, when its input
ends, or when nothing at all has arrived from the engine for `L` seconds (`L` is
several times `E`). It answers `Stopped` if it still can.

Every frame is stamped (`INV-WIRE-001`). A version 4 executor refuses a version 5
frame by name, which is measured.

### 5.4 The executor: three threads

- **Reader.** Frames from stdin onto a channel. It notes the time of the last
  frame, and whether input ended or a `Stop` arrived.
- **Worker.** Carries out one `Perform`. Before each operation it checks the stop
  flag and the liveness deadline.
- **Writer.** The only thing that writes stdout. It serialises `Progress`,
  `Working` and the final frame, so a heartbeat can never interleave inside a
  frame.

At most one `Perform` runs at a time. `Look` and `Build` are answered between
batches.

### 5.5 The engine: running a stage

1. **Group the stage's changes by machine.** Control-domain and `one_at_a_time`
   changes go through the path of step 1.
2. **Record owed reactions first.** For every change in a machine's batch that a
   reaction rouses, record the debt (0026) before the batch is sent.
3. **Send the batches.** Sealing (0041), chunking (§5.8) and a bounded pool of
   machines, limited per route (§5.10).
4. **Consume each stream on its machine's thread.** Forward
   `(machine, change, became)` to the recorder when a change's last operation is
   reported.
5. **The recorder is the only writer of state (0031).** It calls `conclude`,
   records, and writes with group commit (§5.6).
6. **On the first failure anywhere,** start no new batch and send `Stop` to every
   batch in flight. Wait a bounded time for final frames, then report every
   failure, not only the first.
7. **Ctrl-C once:** as in 6, then exit with a failure. **Ctrl-C twice:** exit
   immediately.

nerpa has no signal handling today; the process dies on the first Ctrl-C.

### 5.6 State and `INV-STATE-005`

A change is recorded when the engine learns it completed.

**Group commit.** While one durable write is in progress, completions queue up,
and the next write carries all of them. This keeps the two `fsync`s per write
(§2.1) from multiplying by the number of changes.

**The invariant becomes:** "What a run has made is recorded as soon as the engine
learns of it, and at most the operation in flight is unknown." Today's wording,
"before it touches the next thing", assumed the engine does the touching. In a
batch the executor does, and the honest bound is the operation in flight, not
the change.

### 5.7 Premise

The premise of a batch is the union of its changes' premises, sealed for the
machine and checked before the first operation (0034, 0041). A replacement's
create half no longer needs the empty premise it is given today, because the
check runs before the delete.

### 5.8 Size

A batch is split into chunks by encoded size (8 MiB) and by operation count
(1000), both well under `MAX_FRAME`. Chunks run in order within one conversation.
Changes in a stage are independent, so one chunk cannot invalidate the premise of
the next.

### 5.9 Replacing an installed executor (step 2)

When a machine answers `Build` with another digest, and the fleet names an
installed executor at path `P`:

1. **The plan shows it:** `[replace  ] executor@node  build X → Y`.
2. **Beside, not over.** `Operation::File` writes `dir(P)/.nerpa-executor-<digest>.new`
   with mode 0755. Same directory, so it gets the same SELinux label (§2.4); same
   filesystem, so the rename is atomic.
3. **Health check through the old executor.** `Operation::Process` runs the new
   file with an encoded `Build` request on stdin, in the new file's own protocol
   version. The old executor only pipes bytes. The output must decode to `Build`
   with the carried digest. If it does not, the new file is removed, the machine
   is refused as today, and the old binary was never touched.
4. **Keep the previous, then switch.** `ln -f P P.previous`, then `mv -f new P`.
   The switch is a rename, so there is never a moment without an executor at `P`.
5. **Reconnect and ask `Build` again,** then carry on with the run.

A delivered executor is named by its digest and is simply delivered anew.

**Across a protocol version.** Step 3's engine speaks 5, and the machines it
upgrades run 4. It keeps an encoder for version 4 `Perform` and `Build` frames
only, enough for this sequence. An executor older than the previous version is
refused with manual instructions.

**Signing.** 0010 wants the node to check the binary before replacing it. The
first replacement cannot: the checking code would have to be in the executor
being replaced, which is version 4 and has none.

- **Unsigned in step 2.** It adds no capability: whoever controls the engine can
  already run anything as root through the executor it replaces.
- **Signed from step 3 on.** The version 5 executor carries a verifier
  (`minisign-verify`, MIT, synchronous, no dependencies), so every later
  replacement is checked by the binary being replaced. Key distribution is an
  open question (§8).

### 5.10 Limits the transports impose

| Limit | Value | Consequence | Response |
| --- | --- | --- | --- |
| OpenSSH `MaxStartups` on a shared bastion | `10:30:100` by default | More than 10 unauthenticated connections start being dropped at random. Today's fan-out of 16 through one jump host can exceed it, when reading as well | Concurrency limit per first hop, default 8. Retry pre-authentication drops with jittered backoff. Recommend `ControlMaster` on the jump hop so its authentication is shared |
| OpenSSH `MaxSessions` | 10 per connection | Counts shell, login and subsystem sessions; forwarding channels are not named | Verify before relying on multiplexing through a jump host (§8) |
| Teleport role `max_sessions` | 10 per SSH connection | nerpa opens one session per connection | None needed |
| Teleport proxy `connection_limits.max_connections` | 1000 per client IP, per service | Far above this fleet | Name it in the operator guide |
| One `ssh` process per machine | `W-014` | Memory and process count on the control machine | Unchanged; the pool bound is per route |
| Durable state writes | 2 `fsync` per write, 7.6 ms each | Linear in the number of writes | Group commit (§5.6) |
| Memory | chunk size × machines in flight | 8 MiB × 16 = 128 MiB worst case | Chunk size is the lever |

### 5.11 Failure analysis

| Event | Machine | Engine knows | State says | Next run |
| --- | --- | --- | --- | --- |
| A premise moved | Nothing done | `Moved { at }` | Unchanged | Re-plans |
| An operation fails | Operations before it done | `Progress` up to it, `Failed { at }` | Up to the last reported change | Re-plans; the failed change is proposed again |
| Ctrl-C once | Stops after the operation in flight | `Stopped { at }` from each machine | Everything reported | Continues where it stopped |
| Engine killed, bastion route | Stops at the next operation once input ends (step 3); completes the batch today | Up to the last `Progress` | Up to the last recorded change | Reads the world. Machine resources are found by declared identity: a written file is adopted and a removed one is already gone. Measured: 3000 files laid down outside nerpa, with empty state, planned "nothing to do" |
| Engine killed, Teleport route | Input does not end for up to about a minute (measured 13–69 s); stops when `Alive` is missed for `L` | Same | Same | Same |
| Network partition | Stops when `Alive` is missed for `L` | `ServerAlive*` ends the conversation, and it is reported as unreachable | Up to the last recorded change | The machine is reported with the age of its data (0020) |
| The executor crashes | The operation in flight is unknown | Connection failed | Up to the last recorded change | Reads the world |
| The machine reboots mid-batch | Same | Same | Same | Same. An owed reaction is still owed (0026) |
| The state write fails (disk full) | Batches in flight get `Stop` | Everything reported before the failure | The last successful write | A person fixes the disk; the next run reads the world |
| Fetching a secret fails in `prepare` | No batch sent for that machine | A refusal before sending | Unchanged | As today |
| The replacement fails its health check | Old executor untouched, new file removed | Refused, naming both builds | Unchanged | Refused until fixed |
| The replacement passes, but fails on the next connection | `P.previous` is kept | Refused by `Build` | Unchanged | Manual restore from `P.previous`; named in the refusal |
| Two engines on one machine at once | Two conversations | Each knows its own | The state-file lock protects the file, not the machine | Premises catch overlapping writes; no global lock (§8) |

### 5.12 Testing

This is new policy for `docs/TESTING.md`: the suite has property and adversarial
tests, and no fault injection.

- **An in-memory transport that misbehaves on command:** ends after operation N,
  delays frames, drops `Alive`, answers `Stopped`. The engine is tested against
  every row of §5.11 it can reach without a machine.
- **The real executor behind a pipe,** with input closed mid-batch, no `Alive`
  for `L`, and `Stop` between operations. Each must stop between operations and
  answer what it can.
- **A recorder property:** any order in which completions from several machines
  arrive yields the same state file and the same report (0031's fixed order).
- **Self-update:** the health check refuses a binary for another architecture and
  one of another build; the switch is a rename; the label check on Rocky.
- **The live harness** used for §2.3, run on all three routes, as part of the exit
  check for step 3.

### 5.13 Migration

| Step | Changes | Protocol | Reinstall | Invariants |
| --- | --- | --- | --- | --- |
| 1 | `prepare`/`conclude`; machines in parallel with a `Perform` per change; per-route concurrency limit; handshake retry | 4 | No | `INV-STATE-005` unchanged; `INV-BATCH-001` extended to name applying |
| 2 | Replace an installed executor through itself; health check; `P.previous` | 4 | No | 0040 refined: another build is replaced, not refused, when the path is known |
| 3 | Streamed batches, `Alive`/`Stop`, the three-thread executor, group commit, Ctrl-C, the signature verifier | 5 | Delivered by step 2 | `INV-STATE-005` reworded (§5.6); `W-015` narrowed |

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

## 7. What this costs

- **The provider trait changes.** Today that means the `os` provider and the test
  fakes; later it is paid by every provider author, which is why it happens now.
- **The executor becomes threaded,** and so do the ways it can deadlock.
- **The engine gains signal handling, a recorder thread and a streaming reader.**
- **The wire grows by six frames,** and one previous protocol version is carried
  for upgrades.
- **A replacement runs as root, with no signature on the first one.**
- **A person reading a plan sees a new kind of change:** replacing the executor.

## 8. Open questions

1. **Teleport lingering: answered by measurement on 2026-09-11.** SIGKILL and
   SIGTERM of the local `ssh` gave the same result: the executor lived 13–69 s
   after the batch was sent. So the cause is the route, not an orphaned
   `ProxyCommand`. `Alive` is required, and `L` has to be well under a minute to
   matter on Teleport.
2. **Values for `E`, `L` and `H`.** A slow link must not stop healthy batches, and
   a partition must not keep root processes alive for long.
3. **Multiplexing through a jump host.** Is `MaxSessions` counted for forwarding
   channels?
4. **The signing key:** where it lives, and how the version 5 executor learns the
   public key. Built in, or in a file root installs?
5. **Two engines against one machine.** Is a per-machine lock worth having, and
   where would it live?
6. **Rolling out a change to part of a stage first** (`serial`, canaries). Out of
   scope; it deserves its own RFC.

## 9. Review log

Findings, their verification and their resolution are recorded here.
