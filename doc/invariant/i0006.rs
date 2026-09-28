use crate::invariant::{Enforced, Status};
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-GRAPH-002"),
    status: Status::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("An `after` naming a resource nobody declared, silently ignored instead of refused."), tests: &[crate::anchor::a_dependency_on_a_missing_resource_is_refused] }),
    statement: NonEmptyStr::new("Every declared dependency names a resource that exists in the graph."),
    rationale: r#"A dangling edge means the planner would silently ignore an ordering somebody asked for. Failing closed is the only safe reading."#,
    enforced_by: &[crate::anchor::graph_edges, crate::anchor::graph_stages],
    tests: &[crate::anchor::a_dependency_on_a_missing_resource_is_refused],
);
