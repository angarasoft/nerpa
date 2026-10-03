use dacc_knowledge::{Enforced, InvariantStatus};
use dacc_core::NonEmptyStr;

dacc_knowledge::invariant!("i0026",
    specification: crate::rfc::rfc_2026_002,
    status: InvariantStatus::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("A permission error read as unreachability, which quietly stops managing a resource over a typo in an access rule. One place does this on purpose and is named here rather than left to be found: the fact-gathering loop in `run.rs` turns every failure into unreachability, because there the subject is the machine rather than a resource on it. That makes a protocol error — an executor answering a look with something else — look like a machine that is down, and marks the whole machine unreachable when its resources had already been read. It was decided, it was not written down, and an independent review found it before anybody here did. A second place, also on purpose: opening a machine in the CLI leaves out one that answered, when what answered is an executor of another architecture or another build (0040). The subject there is the machine as well — nothing this nerpa ships is on it to read anything with — and the refusal names it."), tests: &[crate::anchor::any_other_refusal_still_stops_the_run] }),
    statement: NonEmptyStr::new("INV-REACH-002: Only a machine not answering lets a run continue; every other refusal stops it."),
    rationale: r"Treating a permission error as unreachability would quietly stop managing a resource because of a typo in an access rule — the class of silent wrongness this project exists to avoid. The provider has to tell them apart, and the type makes it say which.",
    enforced_by: &[crate::anchor::inv_reach_002_2, crate::anchor::planner_walk, crate::anchor::run_observe],
    tests: &[crate::anchor::any_other_refusal_still_stops_the_run],
);
