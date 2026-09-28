use crate::invariant::{Enforced, Status};
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-PREMISE-001"),
    status: Status::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("An operation whose effect was not derived from anything in the premise — a provider that emits a process without recording what it read to decide on it. The check is only as wide as what the plan admits it looked at, and nothing forces a provider to admit everything, so this bounds how much the guarantee covers rather than offering a way through it. It was once wider than that: the executor held this and nothing produced a premise for it to hold, so every run sent an empty one and the check covered nothing at all. A review found that, and it is why `Provider::read` returns what it looked at rather than only what it found."), tests: &[crate::anchor::a_batch_whose_premise_moved_does_nothing_and_says_which, crate::anchor::a_file_that_moved_after_the_plan_was_made_is_not_written_over] }),
    statement: NonEmptyStr::new("A batch does nothing at all when what the plan was computed from has moved."),
    rationale: r#"Reading everything once, planning once and applying once is what makes central provider logic affordable (`0032`), and it opens a window nobody else's tool has to open this wide: between the looking and the doing, ours is the whole run where Ansible's is one task.

So the batch carries the premise — what the plan was computed from, and what each of those hashed to — and the executor looks again before it does anything. All of it, before any of it: a batch acting on half a stale reading is worse than one that did not act. The digest is computed identically on both ends, from an encoding written out in the protocol rather than borrowed from CBOR.

A premise that no longer holds is answered with `Moved`, not `Failed`. "Look again and plan again" is different advice to "something went wrong", and a person woken at three in the morning should be told which."#,
    enforced_by: &[crate::anchor::executor_perform, crate::anchor::inv_premise_001_2],
    tests: &[crate::anchor::what_it_did_is_read_back_rather_than_echoed],
);
