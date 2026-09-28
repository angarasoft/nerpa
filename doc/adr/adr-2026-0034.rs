use slipway_core::nonempty_str;
use slipway_knowledge::{Breaking, DocStatus};

slipway_knowledge::adr!(
    title: "A plan carries the premise it was computed from",
    status: DocStatus::Active,
    subsystems: &[],
    context: r#"- Corrected: 2026-09-11 — "the same encoding **unkeyed**" put an unkeyed digest of
  whatever was read into the plan, and for a secret already on disk that reached
  the plan's JSON; a test found it. Premise digests are sealed for their machine
  now, and an executor refuses a premise without its seal. See
  [0041](0041-a-premise-is-sealed-for-the-machine-it-describes.md).
## Context

[0006](0006-static-plan-staged-execution.md) reads the whole world once, plans
once, and applies once. That is what makes central provider logic affordable
([0032](0032-provider-logic-stays-on-the-control-machine.md)) — and it opens a
window nobody else's tool has to open this wide.

Between `Look` and `Perform` the machine can change. Ansible's window is one
task; ours is the whole run. A plan that says "the file holds *this*, so write
*that*" is reasoning from a reading that may be minutes old and may be wrong, and
it will act on it without noticing.

The obvious answer — fingerprint the machine and compare — is too blunt. A
touched log or a moved clock invalidates a plan that had nothing to do with
either, and a check that cries wolf gets turned off.

### What it does beyond the window

Signing a plan whose premise cannot be checked is signing *do this, whatever is
there*. Signing a plan with a premise is signing *do this, if the world is still
as described*. The second is what the third-party signature in
[0009](0009-carry-identity-not-keys.md) is for, and the first only resembles it.
So a question that looked like stage 4's has moved to stage 1's protocol.

### What this needs from the code

The canonical encoding of a value lives in `secret.rs` and is private to it. The
premise is its second caller and wants the same encoding **unkeyed** — the
executor has to compute the same digest and holds no key. One encoder in
`value.rs`, two wrappers: keyed for secrets, plain for the premise.
"#,
    decision: r"
The plan carries the **premise**: a digest of every observed value it was
computed from, per machine, and `Perform` carries it to the executor. The
executor re-reads exactly those things, compares, and refuses the batch if any of
them moved — naming the resource and the attribute that did.

This is precise rather than broad because the plan already knows what it read:
every change was decided by comparing specific observed attributes against
specific declared ones. Nothing else is in the premise, so nothing else can
invalidate it. It is optimistic concurrency at the granularity of what was
actually looked at.

It is cheap. Re-reading happens inside `Perform`, so there is no extra journey,
and a machine carrying two hundred resources has a premise of two hundred
digests — six kilobytes.

**The premise is in the protocol from its first version.** Not because the first
executor needs it, but because the wire freezes: an `Ask::Perform` without
somewhere to put it is an `Ask::Perform` that can never have one.
",
    trade_offs: &[r"
The widest correctness gap the single-pass model opens is closed to about the
width every other tool has, and a refusal says what moved instead of failing
somewhere downstream in a way nobody can read.

The costs.

**It is not a lock.** Between the check and the write the world can still change.
The window narrows from the whole run to the operation itself; it does not
disappear, and a record that claimed otherwise would be worse than none.

**It cannot see what nobody read.** A resource outside the configuration is not
in the premise, and drift in it is invisible — which is correct, and is not the
same as safe.

**Secrets do not fit cleanly.** A secret's digest is keyed by the state file, and
the executor has no key. Either the premise digests secret attributes unkeyed —
no leak, since it is a digest of what is on the machine rather than of what is
stored, but a second digest of the same thing under different rules — or secrets
stay out of the premise and are only checked at plan time. The first is more
honest and the bookkeeping is easy to get wrong.

**Every machine pays a re-read.** Cheap in the small, real on a machine holding
many large files, and paid on every apply whether anything moved or not.
"],
    constraints: &[],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: slipway_core::date!(2026, 9, 6),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[],
);
