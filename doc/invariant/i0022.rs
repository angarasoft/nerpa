use crate::invariant::{Enforced, Status};
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-WIRE-002"),
    status: Status::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("A length prefix believed before the allocation it asks for: the number arrives from the far end of a pipe."), tests: &[crate::anchor::an_enormous_announced_frame_is_refused_before_anything_is_allocated, crate::anchor::a_pipe_that_ends_mid_frame_says_so] }),
    statement: NonEmptyStr::new("A frame's announced size is checked before any of it is allocated."),
    rationale: r#"The length prefix is an instruction to allocate that arrives from the other end of a pipe. Believing it is how a peer that is confused, old or hostile turns a number into memory exhaustion. Bounded, and refused above the bound before a byte is reserved."#,
    enforced_by: &[crate::anchor::inv_wire_002],
    tests: &[crate::anchor::an_enormous_announced_frame_is_refused_before_anything_is_allocated, crate::anchor::a_pipe_that_ends_mid_frame_says_so],
);
