//! Ordering, and the two ways a graph can be wrong.

// Tests are allowed to be blunt. An `unwrap` here is an expectation, indexing is
// a claim about shape, and a panic is how a failure gets reported. The rules that
// ban all three apply to code that runs against somebody's infrastructure — see
// docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
use sarma_core::address::address;
use sarma_core::graph::{Graph, GraphError, StageKind};
use sarma_core::{Resource, Target};

fn graph_of(resources: Vec<Resource>) -> Graph {
    let mut graph = Graph::new();
    for resource in resources {
        graph.insert(resource).expect("distinct addresses");
    }
    graph
}

#[test]
fn independent_resources_share_one_stage() {
    let graph = graph_of(vec![
        Resource::new(address("yandex:network.main")),
        Resource::new(address("yandex:disk.spare")),
    ]);

    let stages = graph.stages().expect("no dependencies, no cycles");
    assert_eq!(stages.len(), 1);
    assert_eq!(stages[0].addresses().len(), 2);
    assert_eq!(stages[0].kind(), &StageKind::Control);
}

#[test]
fn a_chain_becomes_one_stage_per_link() {
    let graph = graph_of(vec![
        Resource::new(address("yandex:network.main")),
        Resource::new(address("yandex:subnet.app")).after(address("yandex:network.main")),
        Resource::new(address("yandex:instance.web")).after(address("yandex:subnet.app")),
    ]);

    let stages = graph.stages().expect("acyclic");
    let order: Vec<String> = stages
        .iter()
        .map(|stage| stage.addresses()[0].to_string())
        .collect();

    assert_eq!(
        order,
        [
            "yandex:network.main",
            "yandex:subnet.app",
            "yandex:instance.web"
        ]
    );
}

#[test]
fn a_stage_reports_which_nodes_it_touches() {
    let graph = graph_of(vec![
        Resource::new(address("yandex:instance.web")),
        Resource::new(address("os:package.nginx@web-01")).after(address("yandex:instance.web")),
        Resource::new(address("os:package.nginx@web-02")).after(address("yandex:instance.web")),
    ]);

    let stages = graph.stages().expect("acyclic");
    let StageKind::Target(nodes) = stages[1].kind() else {
        panic!("second stage runs on nodes, got {:?}", stages[1].kind());
    };
    let names: Vec<&str> = nodes.iter().map(Target::as_str).collect();
    assert_eq!(names, ["web-01", "web-02"]);
}

#[test]
fn a_cycle_is_reported_with_every_resource_in_it() {
    let graph = graph_of(vec![
        Resource::new(address("os:file.a@web-01")).after(address("os:file.b@web-01")),
        Resource::new(address("os:file.b@web-01")).after(address("os:file.a@web-01")),
    ]);

    let Err(GraphError::Cycle(stuck)) = graph.stages() else {
        panic!("a two-node cycle must be refused");
    };
    assert_eq!(stuck.len(), 2);
}

/// The cycle report is what somebody debugs at three in the morning. It must not
/// depend on the order the resources happened to be declared in.
#[test]
fn cycle_reports_do_not_depend_on_insertion_order() {
    let one = Resource::new(address("os:file.a@web-01")).after(address("os:file.c@web-01"));
    let two = Resource::new(address("os:file.b@web-01")).after(address("os:file.a@web-01"));
    let three = Resource::new(address("os:file.c@web-01")).after(address("os:file.b@web-01"));

    let forward = graph_of(vec![one.clone(), two.clone(), three.clone()]).stages();
    let backward = graph_of(vec![three, two, one]).stages();

    assert_eq!(forward, backward);
    assert!(matches!(forward, Err(GraphError::Cycle(_))));
}

#[test]
fn a_dependency_on_a_missing_resource_is_refused() {
    let graph = graph_of(vec![
        Resource::new(address("os:service.nginx@web-01")).after(address("os:package.nginx@web-01")),
    ]);

    assert!(matches!(graph.stages(), Err(GraphError::Dangling { .. })));
}

#[test]
fn one_address_cannot_be_declared_twice() {
    let mut graph = Graph::new();
    graph
        .insert(Resource::new(address("yandex:instance.db")))
        .expect("first");

    assert!(matches!(
        graph.insert(Resource::new(address("yandex:instance.db"))),
        Err(GraphError::Duplicate(_))
    ));
}
