use crate::invariant::{Enforced, Status};
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-GRAPH-001"),
    status: Status::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("Edges that close a loop, or two runs disagreeing about which resources a cycle holds up."), tests: &[crate::anchor::a_cycle_is_reported_with_every_resource_in_it, crate::anchor::cycle_reports_do_not_depend_on_insertion_order] }),
    statement: NonEmptyStr::new("The resource graph is acyclic, and a cycle is reported deterministically."),
    rationale: r#"A cycle is a configuration error, not a runtime condition, so it must be found before anything is touched. Reporting it the same way every time matters because the report is what somebody debugs at three in the morning."#,
    enforced_by: &[crate::anchor::graph_stages],
    tests: &[crate::anchor::a_cycle_is_reported_with_every_resource_in_it, crate::anchor::cycle_reports_do_not_depend_on_insertion_order],
);
