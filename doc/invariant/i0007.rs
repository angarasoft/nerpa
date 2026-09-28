use crate::invariant::{Enforced, Status};
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-GRAPH-003"),
    status: Status::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("An attribute reading from another resource with no edge recorded, or one resource read twice and counted as two orderings so it never becomes ready."), tests: &[crate::anchor::two_references_to_one_resource_are_still_one_ordering] }),
    statement: NonEmptyStr::new("An attribute reading from another resource is an edge in the graph."),
    rationale: r"Ordering that depends on somebody remembering to declare it is where ordering bugs come from. If a value is read from another resource, the reader waits for it whether or not anyone noticed.",
    enforced_by: &[crate::anchor::graph_edges, crate::anchor::inv_graph_003_1],
    tests: &[crate::anchor::an_attribute_reading_from_another_resource_is_an_ordering, crate::anchor::two_references_to_one_resource_are_still_one_ordering],
);
