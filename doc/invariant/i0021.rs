use crate::invariant::{Enforced, Status};
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-WIRE-001"),
    status: Status::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("A frame stamped with a version this build does not know, read as though it were a subset of one it does."), tests: &[crate::anchor::a_frame_from_another_version_is_refused_rather_than_guessed_at, crate::anchor::a_frame_from_another_version_is_refused_as_a_version_even_when_what_it_asks_is_new] }),
    statement: NonEmptyStr::new("A frame written by a protocol version this build does not know is refused, not partly believed."),
    rationale: r"An executor built from one version outlives the engine that installed it, so every later engine has to keep talking to it. Every frame carries the version it was written by, and an unknown one is refused rather than read as a subset — guessing at a protocol is how two ends come to disagree about what they agreed.",
    enforced_by: &[crate::anchor::inv_wire_001, crate::anchor::wire_protocol],
    tests: &[crate::anchor::a_frame_from_another_version_is_refused_rather_than_guessed_at, crate::anchor::a_frame_from_another_version_is_refused_as_a_version_even_when_what_it_asks_is_new, crate::anchor::a_frame_of_this_version_asking_for_something_nobody_defined_is_still_refused],
);
