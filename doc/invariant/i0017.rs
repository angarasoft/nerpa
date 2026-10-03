use dacc_knowledge::{Enforced, InvariantStatus};
use dacc_core::NonEmptyStr;

dacc_knowledge::invariant!("i0017",
    specification: crate::rfc::rfc_2026_005,
    status: InvariantStatus::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("Text rendered from anything other than the typed plan, so the page a person approves and the document a machine reads drift apart."), tests: &[crate::anchor::a_plan_survives_a_trip_through_json] }),
    statement: NonEmptyStr::new("INV-AGENT-001: The plan a person reads and the plan a machine reads are the same object."),
    rationale: r"Every machine-readable output in this space is an afterthought bolted onto text meant for people, which is why automating them means parsing prose. Here the text is a rendering of the typed plan and `--json` writes the same object, so the two cannot come to disagree about what the plan says.",
    enforced_by: &[crate::anchor::inv_agent_001_1, crate::anchor::inv_agent_001_2],
    tests: &[crate::anchor::a_plan_survives_a_trip_through_json],
);
