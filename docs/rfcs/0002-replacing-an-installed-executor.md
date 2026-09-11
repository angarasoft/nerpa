# RFC 0002. Replacing an installed executor through itself

- Status: draft; not scheduled
- Date: 2026-09-11
- Split from: [RFC 0001](0001-applying-in-streamed-batches.md), where it was step 2,
  after that RFC's third review
- Touches: 0010, 0033, 0035, 0039, 0040, 0041; `INV-WIRE-001`, `INV-WIRE-003`

## 1. Summary

Since 0040 a new build of nerpa is a reinstall on every machine with an installed
executor, and doing that by hand was already called tedious. This RFC is how
nerpa replaces an installed executor of another build through that executor,
shown in the plan, verified on the engine, and only where a person asked for it
twice.

It is not scheduled (`ROADMAP.md`). It was designed inside RFC 0001 so that the
protocol work there would not rule it out, and three reviews of that design found
enough to fill an RFC of its own. What is below is the design as RFC 0001's second
revision left it, the decisions already made, and the findings that must be
answered before this RFC is reviewed. Section numbers in carried text refer to RFC
0001 at `d6644b1`.

## 2. What was measured

#### 0001 §2.4: Replacing a running executor

These were run on Alpine, against the executor binary itself while it served a
conversation.

- Writing over it is refused with `Text file busy`.
- Renaming another file over it is allowed. The running process continues and
  ends normally when its input closes.

On Rocky SELinux is `Enforcing`. The installed executor is labelled `bin_t`, and
policy gives a new file in `/usr/local/sbin` `bin_t` too. A file moved in from
`/tmp` keeps its temporary label instead.

## 3. What is decided

- **Verified on the engine** against a manifest compiled into nerpa: the full
  digest, architecture and protocol of the executor it carries, a build number
  that only rises, and the manifests of earlier releases. Never a downgrade. A
  development build replaces nothing.
- **Only under `apply`,** and only with both `replace_executor = true` for the
  machine in the fleet and `--replace-executor` on the command line. The plan's
  header lists the machines.
- **Signing the manifest comes later.**
- **Not scheduled.**

## 4. The design as RFC 0001 left it

### The bootstrap

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

#### 0001 §5.10: Replacing an installed executor (step 2, not scheduled)

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

## 5. What must be answered before review

From RFC 0001's third review, logged in its §9 with evidence:

- **X3-8, P3-3.** Every installed executor speaks protocol 4, has no bootstrap and
  exits on the envelope without answering, so the first executor that carries the
  bootstrap is installed by hand. Replacement through protocol 4's `Perform` and
  `Process` is ruled out.
- **X3-1, P3-4.** The frozen set cannot carry the sequence: `Alive`, request ids,
  the frame layer, the premise machinery, removing exactly the `.new` it created,
  a digest of a named file, and the restore.
- **X3-2.** Freeze the sequence, not generic operations: run only the file this
  conversation created, with one fixed `Build` as input; link and rename only among
  `P`, `P.previous` and that `.new`; no stamped frames after the first replacement
  operation.
- **X3-3.** Bind every operation to what was walked: identities carried and checked
  again, a directory lock for the sequence, an exchanging rename.
- **X3-4, X3-9.** Report uid, gid, mode, type, mount identity, filesystem, inode
  flags, ACL presence and the full label; refuse group- or other-writable parts,
  network filesystems, mount points, immutable or append-only attributes; compare
  type and MLS range.
- **X3-5, X3-12.** A rollback the old executor arms at the switch and carries out
  unless a commit arrives; the probe's route slot taken before the switch; a
  replacement is a unit for stopping and for the lock.
- **X3-7.** `plan` shows what would be replaced; the flag names the machines.
- **X3-10, X3-14, X3-15.** `P`'s content tied to `Build` and to a known release;
  what makes a build a release; `P.previous` made unusable after commit.
- **X3-11.** Settings in a fleet table of their own, unknown keys refused.
- **X3-13, P3-5.** `INV-WIRE-001` reworded without losing the version refusal;
  decoding checked to the end of the frame, with tags, indefinite lengths and
  non-minimal encodings refused.
- **X3-16, X3-17.** The risk statement completed; the smaller gaps.

## 6. Review history

Three rounds of review of RFC 0001 covered this design: findings X-1 to X-10,
X2-1 to X2-8 and X3-1 to X3-17, with P-10, P2-2 and P3-3 to P3-5. They and what
became of them are in RFC 0001 §9.
