use dacc_knowledge::{Enforced, InvariantStatus};
use dacc_core::NonEmptyStr;

dacc_knowledge::invariant!("i0018",
    specification: crate::rfc::rfc_2026_002,
    status: InvariantStatus::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("A birth attribute turned into work — which proposes destroying a fleet over an edited image reference — or dropped silently instead of reported."), tests: &[crate::anchor::being_born_with_something_never_becomes_a_replacement, crate::anchor::a_fleet_behind_its_declaration_is_told_about_and_never_planned] }),
    statement: NonEmptyStr::new("INV-BIRTH-001: An attribute a resource was born with is reported, never turned into work."),
    rationale: r"A running machine's boot disk cannot be swapped underneath it, so editing an image reference must not become a proposal to destroy the fleet. But silence is the other failure: `ignore_changes` is how a fleet ends up years behind with nobody aware. Reported on every plan, acted on never. Moving a fleet is a deliberate act. See 0021.",
    enforced_by: &[crate::anchor::inv_birth_001_1, crate::anchor::planner_differences],
    tests: &[crate::anchor::editing_the_image_does_not_propose_to_destroy_anything, crate::anchor::but_it_is_reported_every_time, crate::anchor::being_born_with_something_never_becomes_a_replacement, crate::anchor::a_fleet_behind_its_declaration_is_told_about_and_never_planned],
);
