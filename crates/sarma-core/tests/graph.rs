//! Ordering, and the two ways a graph can be wrong.

// Tests are allowed to be blunt. An `unwrap` here is an expectation, indexing is
// a claim about shape, and a panic is how a failure gets reported. The rules that
// ban all three apply to code that runs against somebody's infrastructure — see
// docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
use sarma_core::address::address;
use sarma_core::graph::{Graph, GraphError, StageKind};
use sarma_core::{Attribute, ExecutionDomain, Provenance, Reference, Resource, Target, Value};

/// Builds a graph and declares every node its resources mention as already
/// existing.
///
/// For tests that are not about where nodes come from. The ones that are build
/// their inventory by hand, because that is what they are testing.
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

#[test]
fn a_graph_reports_what_it_holds() {
    let empty = Graph::new();
    assert!(empty.is_empty());
    assert_eq!(empty.len(), 0);
    assert_eq!(empty.get(&address("yandex:instance.db")), None);

    let graph = graph_of(vec![
        Resource::new(address("yandex:instance.db")).with("cores", 8),
        Resource::new(address("yandex:disk.spare")),
    ]);

    assert!(!graph.is_empty());
    assert_eq!(graph.len(), 2);
    assert_eq!(
        graph
            .get(&address("yandex:instance.db"))
            .and_then(|found| found.attribute("cores"))
            .and_then(Attribute::known),
        Some(&Value::Integer(8))
    );
    assert_eq!(graph.get(&address("yandex:instance.absent")), None);
}

/// The report names what a cycle holds up, and nothing else. Mutation testing
/// found this: every earlier cycle test used a graph where every resource was
/// stuck, so widening the filter to include untangled ones changed no assertion.
#[test]
fn a_cycle_report_leaves_out_resources_it_does_not_hold_up() {
    let graph = graph_of(vec![
        Resource::new(address("os:file.a@web-01")).after(address("os:file.b@web-01")),
        Resource::new(address("os:file.b@web-01")).after(address("os:file.a@web-01")),
        Resource::new(address("yandex:disk.unrelated")),
    ]);

    let Err(GraphError::Cycle(stuck)) = graph.stages() else {
        panic!("the cycle must be refused");
    };
    let named: Vec<String> = stuck.iter().map(ToString::to_string).collect();
    assert_eq!(
        named,
        ["os:file.a@web-01", "os:file.b@web-01"],
        "unrelated resources crept in"
    );
}

#[test]
fn an_attribute_reading_from_another_resource_is_an_ordering() {
    let graph = graph_of(vec![
        Resource::new(address("yandex:instance.db")),
        Resource::new(address("yandex:instance.web")).with(
            "upstream",
            Reference::to(address("yandex:instance.db"), "ip"),
        ),
    ]);

    let stages = graph.stages().expect("acyclic");
    assert_eq!(stages.len(), 2, "the reader waits for what it reads");
    assert_eq!(stages[0].addresses(), [address("yandex:instance.db")]);
    assert_eq!(stages[1].addresses(), [address("yandex:instance.web")]);
}

#[test]
fn two_references_to_one_resource_are_still_one_ordering() {
    let graph = graph_of(vec![
        Resource::new(address("yandex:instance.db")),
        Resource::new(address("yandex:instance.web"))
            .with("host", Reference::to(address("yandex:instance.db"), "ip"))
            .with("port", Reference::to(address("yandex:instance.db"), "port")),
    ]);

    let stages = graph
        .stages()
        .expect("counting an edge twice would strand the reader");
    assert_eq!(stages.len(), 2);
    assert_eq!(stages[1].addresses(), [address("yandex:instance.web")]);
}

#[test]
fn a_reference_to_a_resource_that_is_not_here_is_refused() {
    let graph = graph_of(vec![Resource::new(address("yandex:instance.web")).with(
        "upstream",
        Reference::to(address("yandex:instance.absent"), "ip"),
    )]);

    let Err(GraphError::MissingReference { attribute, .. }) = graph.stages() else {
        panic!("a reference into thin air must be refused");
    };
    assert_eq!(attribute, "upstream");
}

#[test]
fn everything_on_a_node_waits_for_whatever_creates_it() {
    let mut graph = Graph::new();
    graph
        .insert(Resource::new(address("yandex:instance.web")))
        .expect("first");
    graph
        .insert(Resource::new(address("os:package.nginx@web-01")))
        .expect("second");
    graph.managed_by(
        Target::new("web-01").unwrap(),
        address("yandex:instance.web"),
    );

    let stages = graph.stages().expect("acyclic");
    assert_eq!(stages.len(), 2, "the package cannot go first");
    assert_eq!(stages[0].addresses(), [address("yandex:instance.web")]);
    assert_eq!(stages[1].addresses(), [address("os:package.nginx@web-01")]);
}

#[test]
fn a_node_that_was_already_there_adds_no_ordering() {
    let mut graph = Graph::new();
    graph
        .insert(Resource::new(address("os:package.nginx@web-01")))
        .expect("only");
    graph.preexisting(Target::new("web-01").unwrap());

    let stages = graph.stages().expect("acyclic");
    assert_eq!(
        stages.len(),
        1,
        "nothing must happen before touching a machine that exists"
    );
}

/// Fail closed. Ordering a resource last because nobody said where its machine
/// comes from would be a guess dressed up as a decision.
#[test]
fn a_resource_on_a_node_nobody_declared_is_refused() {
    let mut graph = Graph::new();
    graph
        .insert(Resource::new(address("os:package.nginx@web-01")))
        .expect("only");

    let Err(GraphError::UnknownNode { target, .. }) = graph.stages() else {
        panic!("an unplaceable resource must be refused");
    };
    assert_eq!(target.as_str(), "web-01");
}

#[test]
fn the_inventory_reports_what_it_was_told() {
    let mut graph = Graph::new();
    assert!(graph.inventory().is_empty());

    graph.preexisting(Target::new("web-01").unwrap());
    graph.managed_by(
        Target::new("web-02").unwrap(),
        address("yandex:instance.web"),
    );

    assert!(!graph.inventory().is_empty(), "two nodes were declared");
    assert_eq!(graph.inventory().len(), 2);
    assert_eq!(
        graph
            .inventory()
            .provenance(&Target::new("web-02").unwrap()),
        Some(&Provenance::Managed(address("yandex:instance.web")))
    );
    assert_eq!(
        graph
            .inventory()
            .provenance(&Target::new("web-09").unwrap()),
        None
    );
}
