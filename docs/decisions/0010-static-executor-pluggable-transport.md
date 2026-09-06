# 0010. Ship a static executor and treat the transport as a plug

- Status: accepted
- Date: 2026-09-04
- Corrected: 2026-09-07 — "It is signed, and verification runs in both
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

## Decision

A single static Rust binary runs on the target. It speaks framed CBOR over
stdin and stdout, so the transport carries bytes and knows nothing about their
meaning: `teleport`, `ssh`, `local` and `container` are implementations of one
narrow trait, with SSM and serial console possible later.

The executor holds no state beyond a fingerprint of what it applied, and no
secrets: material arrives for the duration of an operation and never touches
disk. It is signed, and verification runs in both directions — the node checks
the binary before replacing it, the engine checks it is talking to the version it
expects.

## Consequences

An identity-aware proxy is a first-class transport rather than a workaround.
Batching many resources into one invocation removes Ansible's per-task round
trip. There is no interpreter on the target, which matters on minimal images.

The cost is a bootstrap problem — the executor has to arrive before it can do
anything — solved three ways depending on the fleet: baked into an image,
delivered signed on first contact, or installed from a package. And a wire
protocol is now a compatibility surface that has to be versioned.
