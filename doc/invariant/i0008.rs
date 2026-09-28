use crate::invariant::{Enforced, Status};
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-GRAPH-004"),
    status: Status::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("A resource on a node the inventory does not know, ordered last as a guess instead of refused."), tests: &[crate::anchor::a_resource_on_a_node_nobody_declared_is_refused] }),
    statement: NonEmptyStr::new("Everything on a node is ordered after whatever brings that node into existence."),
    rationale: r#"A target-domain address names its node as text, and nothing in that text says who creates it. Without the inventory edge the package would be free to run before the machine exists — and a resource whose node nobody declared is refused rather than ordered last, because that would be a guess dressed up as a decision."#,
    enforced_by: &[crate::anchor::graph_edges],
    tests: &[crate::anchor::everything_on_a_node_waits_for_whatever_creates_it, crate::anchor::a_resource_on_a_node_nobody_declared_is_refused, crate::anchor::a_node_that_was_already_there_adds_no_ordering],
);
