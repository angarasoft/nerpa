# RFC 0001. Applying in streamed batches, and an executor that replaces itself

- Status: in review; findings logged in §9, revision pending decisions
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

1. **Teleport lingering: answered by measurement on 2026-09-11.** With a probe every
   second and the local proxy tracked, the proxy exited within a second of `ssh`,
   and the executor lived about 59 s longer. The cause is the route. `Alive` is
   required, and `L` must be well under that to matter on Teleport.
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

| From | How verified | Defect |
| --- | --- | --- |
| S-2 | E | A resource that already matches its declaration is never recorded. Plan and apply say "nothing to do", no state file is written, and when the declaration is removed later nothing is planned: the file stays. |
| S-2 | R | State records only references still unresolved at plan time (`run.rs:411-421`). `after` edges and resolved references are lost, so removal order taken from state is incomplete. |
| S-3 | E | Two addresses with one path are accepted. Both are created, the last write wins, and every later plan proposes the other again. |
| S-4 | R | A reaction's debt is written only when the reacting resource's own stage runs (`run.rs:348`). A run stopped between the cause's stage and that one leaves the new file recorded and the reaction lost. |
| B-4 | R | Nothing checks an operation's size before sending. A file over `MAX_FRAME` plans cleanly and fails at apply. |
| P-6 | R | A package's premise is its query output. Installing A pulls in B and changes B's premise, so a B declared separately in the same stage is refused with `Moved`. Not reproduced on a machine. |
| P-12 | R | A conversation is not dropped after a wire error; only `Drop` clears it. A later ask would read out of step. Latent: nothing asks again today. |
| X-3 | R | The fleet accepts a relative or `~` executor path. `plainly` checks characters, not that the path is absolute. With `become`, that is the executor 0039 exists to refuse. |
| X-8 | R | The number of answers is not checked against the number of operations (`provider-os/src/lib.rs:86-88`). |
| X-10 | D | 0039 says the sudoers rule now "means what it says". An executor running as root runs any process (0033), so the rule is root for that account. 0039 needs a Corrected line. |
| B-6 | R | The executor never syncs what it writes. A machine that loses power can lose a write the engine has recorded; the next read heals it. |

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
| S-2 | blocker | E, R | Adoption does not record, and edges are lost | Defects today, above; stop citing adoption as recovery |
| S-3 | major | E | Stage independence holds in the graph, not on the machine; creates and removals carry no premise | Refuse duplicate identity when the graph is built; re-check each change's own premise before its operations; never split a change |
| S-4 | major | R | The debt is written too late | Before sending the batch that holds a cause, durably owe every reaction it will rouse; `owe` works without a record |
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
