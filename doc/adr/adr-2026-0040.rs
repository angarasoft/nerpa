use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "An executor says which build it is",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    subsystems: &[taxon!(Subsystem, Executor)],
    context: r#"- Corrected: 2026-09-10 — "an executor from before this refuses the first frame
  by name" was written before it was run, and is not what happened. Against a
  machine still holding version 2 the executor said `unknown variant build`: it
  decoded the whole frame before looking at the stamp, so it gave the new request
  as its reason rather than the version. The machine was refused all the same.
  Frames are now read stamp first, so from version 3 on a newer frame is refused
  as a version.
- Corrected: 2026-09-18 — refusing by name is not the same as being heard. An
  executor of protocol 4 handed a version 5 frame writes the refusal to its
  standard error and exits without answering, and the conversation let that
  standard error through to whatever terminal the run had: what reached the
  report was a pipe that broke. It is kept now, to its last 64 KiB, and quoted
  beside the build this nerpa carries — and a connection `ssh` could not make is
  tried again before the machine is called one that will not answer. RFC 0001
  §5.3, P4-9 and P4-11.
## Context

[0039](0039-a-privileged-executor-is-one-the-machine-already-had.md) made the
executor, for any machine worked on with privilege, something root installs
once. Installed things stay where they were put.

Closing stage 1, the three machines it was being closed against were found
running an executor built on 2026-09-06 at 18:50 — the same digest on all three,
and the digest of the static build from that evening. Three fixes to the
executor landed after it, each found by an independent review and each
reproduced first:

- a directory that was a symlink was followed, so a declared mode was `chmod`ed
  onto whatever the link pointed at, as root;
- a file declared `0600` was created world-readable for a moment, and a file's
  content is one of the two places the protocol says a secret goes;
- a process fed a large input deadlocked against its own output.

Every stage-1 run since had been against the build without them, and nothing
said so. It was found by comparing digests by hand for an unrelated reason.

Nothing *could* have said so. `INV-WIRE-001` refuses a frame from a protocol
version this build does not know, and the protocol version did not change,
because none of the three fixes changed a frame. The wire was the same; what was
on the other end of it was not.

## Alternatives

**A revision number, bumped by hand when the executor changes.** Exactly what
nobody did. Nothing on the wire changed on 2026-09-06, so there was nothing to
prompt it, and a check that depends on remembering is the check that failed.

**A digest of the executor's sources, computed at build time.** Closer to "did
anything change", and it needs one crate's build script reading another crate's
sources — a fragile foundation for a security property. It would also call two
binaries the same that different compilers made from the same source.

**Warn and carry on.** A warning in a tool that prints a plan is a line nobody
reads; 0039 refused a sudoers rule for the same reason.

**Stop the whole run.** One machine forgotten in a fleet of two hundred would stop
the other hundred and ninety-nine, which is how people learn to switch a check
off. The machine is reported by name and its resources as unread, which is what
[0020](0020-a-machine-that-cannot-be-reached.md) already does for a
machine that is down.
"#,
    decision: r"
**The executor answers `Ask::Build` with a digest of its own running binary, and
every machine reached over `ssh` is asked before anything else is.** The engine
computes the same digest over the `nerpa-executor.static` it carries. A machine
that names another build, or cannot say, is left out of the run and reported
with both names — the way a machine of the wrong architecture is at delivery.

The digest is blake3 over the bytes of `/proc/self/exe`, which is the file the
process was started from even if it has since been replaced on disk.

The protocol goes to version 3, so an executor from before this refuses the
first frame by name — *written by protocol version 3, and this is version 2* —
rather than being asked a question it cannot parse.

Opening a machine now waits for its handshake, so machines are opened side by
side over the bounded pool rather than one after another.
",
    trade_offs: &[r"
A new build of nerpa is a reinstall on every machine with an installed executor,
including a build where nothing in the executor's source changed but the
compiler did. That is the cost of naming a build by its bytes. It is paid by
the person upgrading, once, instead of by a fleet quietly running what the
upgrade was for. Delivered executors are unaffected: they are already named by a
digest of themselves, and a new one is put there.

Nerpa now needs `nerpa-executor.static` beside it for every machine reached over
`ssh`, not only the machines it delivers to. Without it, those machines are
refused by name.

`INV-REACH-002` says only a machine not answering lets a run continue. Opening a
machine now refuses one that answered, and that is named in its bypass rather
than left for somebody to find.

The local route is not asked. The executor there is the file beside this
program, from the same build by construction.

Not settled: an executor packaged by a distribution is a different build from
nerpa's own for the same source. When that exists, the question becomes which
builds a nerpa accepts rather than which one it carries — a list, not a name.
"],
    constraints: &[],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 10),
    breaking: Breaking::No,
    introduces_norm: false,
    code_refs: &[],
    related_rfcs: &[crate::rfc::rfc_2026_003],
);
