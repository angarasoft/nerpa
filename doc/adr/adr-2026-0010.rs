use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "Ship a static executor and treat the transport as a plug",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    subsystems: &[taxon!(Subsystem, Executor)],
    context: r#"- Corrected: 2026-09-07 — "It is signed, and verification runs in both
  directions" is written below in the present tense and nothing signs anything.
  There is no signing primitive in the dependency tree; delivery names the binary
  by a digest of itself, which tells a machine whether it already has this build
  and tells it nothing about who made it. An independent review found the claim
  before anybody here noticed it had never been true. The sentence stands because
  a record is not edited; what it describes is not built, and `ARCHITECTURE.md`
  now says so.
## Context

Ansible is slow because it orchestrates Python over SSH: one task is one round
trip, plus fact gathering on top. It also assumes SSH with a key, so an
identity-aware proxy has to be bolted on through configuration files — a shape
this author has already fought through on live infrastructure.
"#,
    decision: r"
A single static Rust binary runs on the target. It speaks framed CBOR over
stdin and stdout, so the transport carries bytes and knows nothing about their
meaning: `teleport`, `ssh`, `local` and `container` are implementations of one
narrow trait, with SSM and serial console possible later.

The executor holds no state beyond a fingerprint of what it applied, and no
secrets: material arrives for the duration of an operation and never touches
disk. It is signed, and verification runs in both directions — the node checks
the binary before replacing it, the engine checks it is talking to the version it
expects.
",
    trade_offs: &[r"
An identity-aware proxy is a first-class transport rather than a workaround.
Batching many resources into one invocation removes Ansible's per-task round
trip. There is no interpreter on the target, which matters on minimal images.

The cost is a bootstrap problem — the executor has to arrive before it can do
anything — solved three ways depending on the fleet: baked into an image,
delivered signed on first contact, or installed from a package. And a wire
protocol is now a compatibility surface that has to be versioned.
"],
    constraints: &[],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 4),
    breaking: Breaking::No,
    introduces_norm: false,
    code_refs: &[crate::anchor::wire_protocol],
    related_rfcs: &[crate::rfc::rfc_2026_003],
);
