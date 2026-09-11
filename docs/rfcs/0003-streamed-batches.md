# RFC 0003. Streamed batches

- Status: draft; to be written in full once step 1 of RFC 0001 is built and measured
- Date: 2026-09-11
- Split from: [RFC 0001](0001-applying-in-streamed-batches.md), where it was step 3,
  after that RFC's third review
- Touches: 0010, 0031, 0033, 0034, 0041; `INV-WIRE-001`, `INV-PREMISE-001`,
  `W-015`

## 1. Summary

Applying costs one network exchange per change. Batching a machine's changes for a
stage into one conversation that reports every operation as it completes gains
16–22 times for many small changes and 1.5 times for large ones, measured in RFC
0001. This RFC is the protocol for that: streamed answers, liveness in both
directions, and stopping at a change boundary.

It waits for step 1 of RFC 0001: machines in parallel, the recorder and shards,
still one `Perform` per change. Whether batching is still worth its cost is
decided by measuring what step 1 alone gains. What is below is the design as RFC
0001's second revision left it, and the findings to answer when this RFC is written.
Section numbers in carried text refer to RFC 0001 at `d6644b1`.

## 2. What was measured

#### 0001 §2.1: Where the time of one applied change goes

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

#### 0001 §2.3: What an executor does when its engine disappears

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

## 3. The design as RFC 0001 left it

#### 0001 §5.3: Protocol 5

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

#### 0001 §5.4: The executor: three threads

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

#### 0001 §5.8: Premise

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

#### 0001 §5.9: Size

A batch is split into chunks by encoded size (8 MiB) and by operation count
(1000), **only between changes**, well under `MAX_FRAME`. Chunks run in order in
one conversation, and the premise is checked before the first.

**A change's own encoded size is bounded while planning:** the total of its
operations, where `INV-PLAN-007` bounds one attribute today. A change too large
for a frame is refused before anything is sent; one larger than a chunk is sent as
a chunk of its own, up to that bound.

## 4. What must be answered when this is written

From RFC 0001's third review, logged in its §9 with evidence:

- **P3-6.** Engine-side liveness while a conversation is idle, and both timers
  counted from the last byte rather than the last frame, bounded below by the
  largest frame over the slowest route.
- **P3-7.** After `Moved`, or a `Failed` on a premise, the rest is resent with its
  own premises, because nothing ran and later premises were never evaluated.
- **P3-8.** Entries of changes that will be resent are kept, not withdrawn.
- **P3-9.** Every `Refused` is a contract violation; an undecodable frame gets none.
- **P3-10.** A conversation-level `Stopped` after any final frame; change starts in
  `Perform`; position rules for every final frame; rules for `Look` and `Build`.
- **P3-11.** A panic aborts through a hook; the end of input sets the stop flag and
  the executor exits at a change boundary.
- **P3-14, P3-18.** The failure table's streaming rows; one name for `seq`.
- **B3-2.** Three threads per open conversation for the whole run scale with the
  machines carrying work: stated with a figure, or conversations bounded.
- **The partly resolved second-round findings** P2-3, P2-4, P2-7, P2-10, P2-12 and
  P2-13, as the third review's Section A left them.

## 5. Review history

The design was reviewed three times inside RFC 0001: findings P-1 to P-14, P2-1 to
P2-19 and P3-1 to P3-18, with B-7, B3-2 and C-5. They and what became of them are in
RFC 0001 §9.
