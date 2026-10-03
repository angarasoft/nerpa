use dacc_knowledge::{Enforced, InvariantStatus};
use dacc_core::NonEmptyStr;

dacc_knowledge::invariant!("i0004",
    specification: crate::rfc::rfc_2026_002,
    status: InvariantStatus::Enforced(Enforced::Unrepresentable),
    statement: NonEmptyStr::new("INV-DOMAIN-001: Execution domain is derived from the address, never stored beside it."),
    rationale: r"A separate field is a second source of truth, and the two can disagree. The planner would then look for a node the address says is not there, or skip one that is.",
    enforced_by: &[crate::anchor::inv_domain_001],
    tests: &[crate::anchor::control_resources_have_no_node, crate::anchor::target_resources_name_the_node_they_live_on],
);
