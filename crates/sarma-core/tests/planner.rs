//! What a plan says, and the one property it must have twice.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::BTreeMap;

use sarma_core::address::address;
use sarma_core::planner::plan;
use sarma_core::provider::Attributes;
use sarma_core::state::{Record, State};
use sarma_core::{
    Change, ChangeKind, Confidence, ExecutionDomain, Graph, Kind, Observed, Provider, Reference,
    Resource, Schema, Target, Value,
};

fn graph_of(resources: Vec<Resource>) -> Graph {
    let mut graph = Graph::new();
    for resource in resources {
        if let ExecutionDomain::Target(target) = resource.address().domain() {
            graph.preexisting(target);
        }
        graph.insert(resource).expect("distinct addresses");
    }
    graph
}

fn observed_with(entries: &[(&str, &[(&str, Value)])]) -> Observed {
    let mut observed = Observed::new();
    for (address_text, attributes) in entries {
        let mut recorded = BTreeMap::new();
        for (name, value) in *attributes {
            recorded.insert((*name).to_owned(), value.clone());
        }
        observed.record(address(address_text), recorded);
    }
    observed
}

fn only_change(graph: &Graph, observed: &Observed, schema: &Schema) -> Change {
    let plan = plan(graph, observed, &State::new(), schema).expect("plannable");
    let changes: Vec<&Change> = plan.changes().collect();
    assert_eq!(
        changes.len(),
        1,
        "expected exactly one change, got {changes:?}"
    );
    changes[0].clone()
}

#[test]
fn a_resource_that_does_not_exist_is_created() {
    let graph = graph_of(vec![
        Resource::new(address("yandex:instance.db")).with("cores", 8),
    ]);

    let change = only_change(&graph, &Observed::new(), &Schema::new());
    assert_eq!(change.kind(), ChangeKind::Create);
    assert_eq!(change.deltas().len(), 1);
    assert_eq!(change.deltas()[0].before(), None);
    assert_eq!(
        change.deltas()[0].after(),
        &Confidence::Known(Value::Integer(8))
    );
}

#[test]
fn a_resource_that_already_matches_produces_nothing() {
    let graph = graph_of(vec![
        Resource::new(address("yandex:instance.db")).with("cores", 8),
    ]);
    let observed = observed_with(&[("yandex:instance.db", &[("cores", Value::Integer(8))])]);

    let plan = plan(&graph, &observed, &State::new(), &Schema::new()).expect("plannable");
    assert!(plan.is_empty(), "a converged world needs no plan: {plan:?}");
    assert_eq!(plan.blast_radius().total(), 0);
}

#[test]
fn a_changed_attribute_is_an_update_carrying_both_values() {
    let graph = graph_of(vec![
        Resource::new(address("yandex:instance.db")).with("cores", 8),
    ]);
    let observed = observed_with(&[("yandex:instance.db", &[("cores", Value::Integer(4))])]);

    let change = only_change(&graph, &observed, &Schema::new());
    assert_eq!(change.kind(), ChangeKind::Update);
    assert!(change.kind().is_reversible());
    assert_eq!(change.deltas()[0].before(), Some(&Value::Integer(4)));
    assert_eq!(
        change.deltas()[0].after(),
        &Confidence::Known(Value::Integer(8))
    );
}

#[test]
fn an_attribute_that_cannot_change_in_place_makes_it_a_replacement_and_says_which() {
    let graph = graph_of(vec![
        Resource::new(address("yandex:instance.db"))
            .with("zone", "ru-central1-e")
            .with("cores", 8),
    ]);
    let observed = observed_with(&[(
        "yandex:instance.db",
        &[
            ("zone", Value::Text("ru-central1-a".to_owned())),
            ("cores", Value::Integer(4)),
        ],
    )]);
    let schema = Schema::new().immutable(
        Provider::new("yandex").unwrap(),
        Kind::new("instance").unwrap(),
        "zone",
    );

    let change = only_change(&graph, &observed, &schema);
    assert_eq!(change.kind(), ChangeKind::Replace);
    assert!(!change.kind().is_reversible());
    assert_eq!(
        change.cause().expect("a replacement has a cause").name(),
        "zone"
    );
}

/// A reference is unresolved in the text, not in the world. If the resource it
/// reads from already exists, the value is known now — and without that nothing
/// reading another resource would ever converge.
#[test]
fn a_reference_to_something_that_exists_is_already_known() {
    let graph = graph_of(vec![
        Resource::new(address("yandex:instance.db")),
        Resource::new(address("yandex:instance.web")).with(
            "upstream",
            Reference::to(address("yandex:instance.db"), "ip"),
        ),
    ]);

    let unknown = plan(&graph, &Observed::new(), &State::new(), &Schema::new()).expect("plannable");
    assert_eq!(unknown.blast_radius().unresolved(), 1);

    let observed = observed_with(&[
        (
            "yandex:instance.db",
            &[("ip", Value::Text("10.0.0.1".to_owned()))],
        ),
        (
            "yandex:instance.web",
            &[("upstream", Value::Text("10.0.0.1".to_owned()))],
        ),
    ]);
    let settled = plan(&graph, &observed, &State::new(), &Schema::new()).expect("plannable");
    assert!(
        settled.is_empty(),
        "once the source exists, the reader matches: {settled:?}"
    );
}

/// The property this tool exists to have. Applying a plan and planning again
/// must produce nothing at all.
#[test]
fn planning_after_applying_produces_an_empty_plan() {
    let graph = graph_of(vec![
        Resource::new(address("yandex:instance.db")).with("cores", 8),
        Resource::new(address("os:package.nginx@web-01")).with("state", "present"),
        Resource::new(address("os:file.nginx.conf@web-01"))
            .with("listen", Reference::to(address("yandex:instance.db"), "ip")),
    ]);

    let first = plan(&graph, &Observed::new(), &State::new(), &Schema::new()).expect("plannable");
    assert_eq!(first.blast_radius().creates(), 3);
    assert_eq!(first.blast_radius().unresolved(), 1);

    // What the world looks like once that plan has been carried out.
    let applied = observed_with(&[
        (
            "yandex:instance.db",
            &[
                ("cores", Value::Integer(8)),
                ("ip", Value::Text("10.0.0.1".to_owned())),
            ],
        ),
        (
            "os:package.nginx@web-01",
            &[("state", Value::Text("present".to_owned()))],
        ),
        (
            "os:file.nginx.conf@web-01",
            &[("listen", Value::Text("10.0.0.1".to_owned()))],
        ),
    ]);

    let second = plan(&graph, &applied, &State::new(), &Schema::new()).expect("plannable");
    assert!(
        second.is_empty(),
        "the second run must change nothing: {second:?}"
    );
}

#[test]
fn the_blast_radius_names_every_machine_and_counts_what_cannot_be_undone() {
    let graph = graph_of(vec![
        Resource::new(address("yandex:instance.db")).with("zone", "e"),
        Resource::new(address("os:package.nginx@web-01")),
        Resource::new(address("os:package.nginx@web-02")),
    ]);
    let observed = observed_with(&[(
        "yandex:instance.db",
        &[("zone", Value::Text("a".to_owned()))],
    )]);
    let schema = Schema::new().immutable(
        Provider::new("yandex").unwrap(),
        Kind::new("instance").unwrap(),
        "zone",
    );

    let plan = plan(&graph, &observed, &State::new(), &schema).expect("plannable");
    let radius = plan.blast_radius();

    assert_eq!(radius.creates(), 2);
    assert_eq!(radius.replaces(), 1);
    assert_eq!(radius.updates(), 0);
    assert_eq!(radius.total(), 3);
    assert!(radius.has_irreversible());
    let touched: Vec<&str> = radius.nodes().iter().map(Target::as_str).collect();
    assert_eq!(touched, ["web-01", "web-02"]);
}

#[test]
fn stages_with_nothing_to_do_are_left_out_but_order_is_kept() {
    let graph = graph_of(vec![
        Resource::new(address("yandex:network.main")).with("name", "main"),
        Resource::new(address("yandex:subnet.app")).with(
            "network_id",
            Reference::to(address("yandex:network.main"), "id"),
        ),
    ]);
    let observed = observed_with(&[(
        "yandex:network.main",
        &[
            ("name", Value::Text("main".to_owned())),
            ("id", Value::Text("net-1".to_owned())),
        ],
    )]);

    let plan = plan(&graph, &observed, &State::new(), &Schema::new()).expect("plannable");
    assert_eq!(
        plan.stages().len(),
        1,
        "the settled network is not a stage anybody reads"
    );
    assert_eq!(
        plan.stages()[0].changes()[0].address(),
        &address("yandex:subnet.app")
    );
}

#[test]
fn a_graph_that_cannot_be_ordered_is_not_planned_at_all() {
    let graph = graph_of(vec![
        Resource::new(address("os:file.a@web-01")).after(address("os:file.b@web-01")),
        Resource::new(address("os:file.b@web-01")).after(address("os:file.a@web-01")),
    ]);

    assert!(plan(&graph, &Observed::new(), &State::new(), &Schema::new()).is_err());
}

#[test]
fn observed_state_reports_what_it_holds() {
    let empty = Observed::new();
    assert!(empty.is_empty());
    assert_eq!(empty.len(), 0);
    assert!(!empty.contains(&address("yandex:instance.db")));
    assert_eq!(empty.addresses().count(), 0);

    let observed = observed_with(&[
        ("yandex:instance.db", &[("cores", Value::Integer(8))]),
        ("yandex:disk.spare", &[]),
    ]);

    assert!(!observed.is_empty());
    assert_eq!(observed.len(), 2);
    assert!(observed.contains(&address("yandex:instance.db")));
    assert!(!observed.contains(&address("yandex:instance.absent")));
    assert_eq!(
        observed.attribute(&address("yandex:instance.db"), "cores"),
        Some(&Value::Integer(8))
    );
    assert_eq!(
        observed.attribute(&address("yandex:instance.db"), "zone"),
        None
    );

    let found: Vec<String> = observed.addresses().map(ToString::to_string).collect();
    assert_eq!(found, ["yandex:disk.spare", "yandex:instance.db"]);
}

/// Each figure asserted where it is zero as well as where it is not: a counter
/// that always returns the number this test happens to expect is not a counter.
#[test]
fn the_blast_radius_counts_each_kind_separately() {
    let graph = graph_of(vec![
        Resource::new(address("yandex:instance.new")),
        Resource::new(address("yandex:instance.changed")).with("cores", 8),
    ]);
    let observed = observed_with(&[("yandex:instance.changed", &[("cores", Value::Integer(4))])]);

    let radius = plan(&graph, &observed, &State::new(), &Schema::new())
        .expect("plannable")
        .blast_radius()
        .clone();
    assert_eq!(radius.creates(), 1);
    assert_eq!(radius.updates(), 1);
    assert_eq!(radius.replaces(), 0);
    assert_eq!(radius.unresolved(), 0);
    assert_eq!(radius.total(), 2);
    assert!(!radius.has_irreversible(), "nothing here destroys anything");
    assert!(radius.nodes().is_empty());
}

#[test]
fn an_empty_plan_is_empty_and_a_plan_with_work_is_not() {
    let graph = graph_of(vec![
        Resource::new(address("yandex:instance.db")).with("cores", 8),
    ]);

    let work = plan(&graph, &Observed::new(), &State::new(), &Schema::new()).expect("plannable");
    assert!(!work.is_empty(), "there is a resource to create");
    assert_eq!(work.changes().count(), 1);

    let settled = observed_with(&[("yandex:instance.db", &[("cores", Value::Integer(8))])]);
    let nothing = plan(&graph, &settled, &State::new(), &Schema::new()).expect("plannable");
    assert!(nothing.is_empty());
    assert_eq!(nothing.changes().count(), 0);
}

/// Removals count towards the total like everything else. Asserted with other
/// kinds present, so that adding them wrongly cannot cancel out.
#[test]
fn the_total_counts_removals_alongside_everything_else() {
    let graph = graph_of(vec![
        Resource::new(address("yandex:instance.new")),
        Resource::new(address("yandex:disk.also_new")),
    ]);

    let mut state = State::new();
    state.record(
        address("yandex:instance.gone"),
        Record::new(Attributes::new(), std::collections::BTreeSet::new()),
    );

    let plan = plan(&graph, &Observed::new(), &state, &Schema::new()).expect("plannable");
    let radius = plan.blast_radius();

    assert_eq!(radius.creates(), 2);
    assert_eq!(radius.deletes(), 1);
    assert_eq!(
        radius.total(),
        3,
        "two made and one removed is three changes"
    );
}
