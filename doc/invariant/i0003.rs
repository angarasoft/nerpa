use crate::invariant::{Enforced, Status};
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-ADDR-003"),
    status: Status::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("A shape that prints differently from the way it parsed: a name holding a separator, a component the grammar admits but Display does not reproduce. A name now holds `/` on purpose, because in portage a package is `category/name` — so the shape the bypass names is a real shape rather than a hypothetical one."), tests: &[crate::anchor::every_shape_round_trips, crate::anchor::a_name_and_a_slot_may_hold_a_slash_because_somebody_elses_namespace_does] }),
    statement: NonEmptyStr::new("Address text round-trips: parsing then printing yields the input."),
    rationale: r"The plan hash is taken over canonical text. A value that printed differently from the way it parsed would produce two hashes for one plan and invalidate a signature that was given in good faith.",
    enforced_by: &[crate::anchor::inv_addr_003],
    tests: &[crate::anchor::every_shape_round_trips],
);
