use crate::invariant::{Enforced, Status};
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-GRAPH-005"),
    status: Status::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("Any `Result` from the graph passed through `flatten`, `ok()`, `unwrap_or_default` or `iter().flatten()` on the way into a collection. Each reads as tidy iterator code and each silently substitutes an empty configuration for a refusal — which is how this one arrived."), tests: &[crate::anchor::a_graph_that_cannot_be_ordered_stops_the_run_rather_than_reading_as_empty] }),
    statement: NonEmptyStr::new("A configuration that cannot be ordered stops the run, and never reads as a configuration that declares nothing."),
    rationale: r"`Graph::stages` returns a `Result`, and every convenient way of getting past a `Result` in an iterator chain — `flatten`, `ok`, `unwrap_or_default` — turns the error into an empty sequence. A cycle then reads as a configuration with no resources in it: the run continues, looks only at what state remembers, and is wrong about what it is doing rather than stopped. The graph is the thing being asked about, so failing to derive it is the end of the question.",
    enforced_by: &[crate::anchor::inv_graph_005],
    tests: &[crate::anchor::every_resource_on_one_machine_is_asked_for_at_once, crate::anchor::something_only_state_remembers_travels_with_the_rest],
);
