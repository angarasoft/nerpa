use crate::invariant::{Enforced, Status};
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-DOMAIN-001"),
    status: Status::Enforced(Enforced::Unrepresentable),
    statement: NonEmptyStr::new("Execution domain is derived from the address, never stored beside it."),
    rationale: r"A separate field is a second source of truth, and the two can disagree. The planner would then look for a node the address says is not there, or skip one that is.",
    enforced_by: &[crate::anchor::inv_domain_001],
    tests: &[crate::anchor::control_resources_have_no_node, crate::anchor::target_resources_name_the_node_they_live_on],
);
