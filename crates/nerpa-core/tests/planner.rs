//! What a plan says, and the one property it must have twice.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::{BTreeMap, BTreeSet};

use nerpa_core::address::address;
use nerpa_core::planner::{Because, PlanError, Step, plan};
use nerpa_core::provider::Attributes;
use nerpa_core::secret::Recorded;
use nerpa_core::secret::{Key, Remembered, Secrets};
use nerpa_core::state::{Record, State};
use nerpa_core::{
    Change, ChangeKind, Confidence, DeltaKind, ExecutionDomain, Graph, Kind, Observed, Provider,
    Reference, Resource, Schema, Target, Value,
};

/// A fixed key and a store holding nothing.
///
/// No test in this file declares a secret; planning still has to be told where
/// one would be fetched from, and a fixed key keeps digests reproducible.
static DIGEST_KEY: Key = Key::from_bytes([7; 32]);
static NO_STORE: Remembered = Remembered::EMPTY;

fn secrets() -> Secrets<'static> {
    Secrets::new(&DIGEST_KEY, &NO_STORE)
}

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

/// A state recording each of these, with the edges `graph` gives them, as the run
/// that made them would have left it.
///
/// A world that matches its declaration is converged only once it is recorded as
/// well, and recorded with what each resource comes after: one that nothing
/// records is adopted, and one recorded with other edges is reordered, and both
/// are work. See decisions 0042 and 0043.
fn recorded(graph: &Graph, addresses: &[&str]) -> State {
    let mut state = State::new();
    for address_text in addresses {
        let address = address(address_text);
        let edges = graph.dependencies_of(&address).expect("orderable");
        state.record(address, Record::new(BTreeMap::new(), edges));
    }
    state
}

/// A schema that says what the os provider says about paths.
fn os_schema() -> Schema {
    let os = || Provider::new("os").unwrap();
    Schema::new()
        .immutable(os(), Kind::new("file").unwrap(), "path")
        .immutable(os(), Kind::new("directory").unwrap(), "path")
        .identity(os(), Kind::new("file").unwrap(), "path", "path")
        .identity(os(), Kind::new("directory").unwrap(), "path", "path")
}

/// A record of these plain values, after these addresses.
fn record_of(values: &[(&str, &str)], after: &[&str]) -> Record {
    Record::new(
        Recorded::all(
            values
                .iter()
                .map(|(name, value)| ((*name).to_owned(), Value::Text((*value).to_owned())))
                .collect::<Attributes>(),
        ),
        after.iter().map(|each| address(each)).collect(),
    )
}

/// Each stage's changes, by address and kind.
fn order_of(plan: &nerpa_core::Plan) -> Vec<Vec<(String, ChangeKind)>> {
    plan.stages()
        .iter()
        .map(|stage| {
            stage
                .changes()
                .iter()
                .map(|change| (change.address().to_string(), change.kind()))
                .collect()
        })
        .collect()
}

fn only_change(graph: &Graph, observed: &Observed, schema: &Schema) -> Change {
    let plan = plan(graph, observed, &State::new(), schema, secrets()).expect("plannable");
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
#[dacc_derive::doc_anchor(id = "a-resource-that-already-matches-produces-nothing")]
fn a_resource_that_already_matches_produces_nothing() {
    let graph = graph_of(vec![
        Resource::new(address("yandex:instance.db")).with("cores", 8),
    ]);
    let observed = observed_with(&[("yandex:instance.db", &[("cores", Value::Integer(8))])]);

    let plan = plan(
        &graph,
        &observed,
        &recorded(&graph, &["yandex:instance.db"]),
        &Schema::new(),
        secrets(),
    )
    .expect("plannable");
    assert!(plan.is_empty(), "a converged world needs no plan: {plan:?}");
    assert_eq!(plan.blast_radius().total(), 0);
}

/// A recorded resource that matches and has been given an edge since is reordered:
/// nothing is done to it, and it is recorded again with the edge, which is what
/// its removal will be ordered by. Planned as nothing, the edge never reached
/// state. Decision 0043.
#[test]
#[dacc_derive::doc_anchor(id = "a-resource-given-a-new-edge-is-reordered-and-nothing-else")]
fn a_resource_given_a_new_edge_is_reordered_and_nothing_else() {
    let graph = graph_of(vec![
        Resource::new(address("yandex:instance.db")).with("cores", 8),
        Resource::new(address("yandex:instance.web"))
            .with("cores", 2)
            .after(address("yandex:instance.db")),
    ]);
    let observed = observed_with(&[
        ("yandex:instance.db", &[("cores", Value::Integer(8))]),
        ("yandex:instance.web", &[("cores", Value::Integer(2))]),
    ]);
    // As a run from before the edge was written in left it.
    let before_the_edge = recorded(
        &Graph::new(),
        &["yandex:instance.db", "yandex:instance.web"],
    );

    let reordering = plan(
        &graph,
        &observed,
        &before_the_edge,
        &Schema::new(),
        secrets(),
    )
    .expect("plannable");
    let changes: Vec<&Change> = reordering.changes().collect();
    assert_eq!(
        changes.len(),
        1,
        "only the one given an edge: {reordering:?}"
    );
    assert_eq!(changes[0].address(), &address("yandex:instance.web"));
    assert_eq!(changes[0].kind(), ChangeKind::Reorder);
    assert_eq!(changes[0].depends_on(), [address("yandex:instance.db")]);
    assert!(changes[0].deltas().is_empty(), "nothing differs");
    assert!(changes[0].premise().is_empty(), "and nothing is sent");
    assert_eq!(reordering.blast_radius().reorders(), 1);

    let with_the_edge = recorded(&graph, &["yandex:instance.db", "yandex:instance.web"]);
    let settled =
        plan(&graph, &observed, &with_the_edge, &Schema::new(), secrets()).expect("plannable");
    assert!(settled.is_empty(), "once recorded, nothing: {settled:?}");
}

/// A declared resource that names what a removal takes away is made again after
/// the removal, not read from before it. Renaming an address and keeping its
/// path used to remove the file and adopt the new resource from the reading taken
/// before the removal. Found by the second review of RFC 0001.
#[test]
#[dacc_derive::doc_anchor(id = "a-resource-naming-what-a-removal-takes-is-created-after-it")]
fn a_resource_naming_what_a_removal_takes_is_created_after_it() {
    let schema = Schema::new().identity(
        Provider::new("os").unwrap(),
        Kind::new("file").unwrap(),
        "path",
        "path",
    );
    let graph = graph_of(vec![
        Resource::new(address("os:file.new@web-01"))
            .with("path", "/etc/motd")
            .with("content", "hello\n"),
    ]);
    let observed = observed_with(&[(
        "os:file.new@web-01",
        &[
            ("path", Value::Text("/etc/motd".to_owned())),
            ("content", Value::Text("hello\n".to_owned())),
        ],
    )]);
    let mut state = State::new();
    state.record(
        address("os:file.old@web-01"),
        Record::new(
            Recorded::all(
                [("path".to_owned(), Value::Text("/etc//motd".to_owned()))]
                    .into_iter()
                    .collect::<Attributes>(),
            ),
            BTreeSet::new(),
        ),
    );

    let plan = plan(&graph, &observed, &state, &schema, secrets()).expect("plannable");
    let kinds: Vec<(String, ChangeKind)> = plan
        .changes()
        .map(|change| (change.address().to_string(), change.kind()))
        .collect();
    assert_eq!(
        kinds,
        [
            ("os:file.old@web-01".to_owned(), ChangeKind::Delete),
            ("os:file.new@web-01".to_owned(), ChangeKind::Create),
        ],
        "removed, then made again rather than adopted from before: {plan:?}"
    );
}

/// A removal comes after the change of whatever state recorded after it that
/// moves out of it, not before everything. A file recorded inside its directory and
/// moved out of it while the directory's declaration went was still inside when
/// the directory was removed; the removal refused, and every later run did the
/// same. Found by the narrow review of RFC 0001.
#[test]
#[dacc_derive::doc_anchor(id = "a-removal-comes-after-the-change-of-what-was-recorded-after-it")]
fn a_removal_comes_after_the_change_of_what_was_recorded_after_it() {
    let graph = graph_of(vec![
        Resource::new(address("os:file.conf@web-01"))
            .with("path", "/conf")
            .with("content", "moved\n"),
        Resource::new(address("os:file.motd@web-01"))
            .with("path", "/motd")
            .with("content", "hello\n"),
    ]);
    let observed = observed_with(&[
        (
            "os:file.conf@web-01",
            &[
                ("path", Value::Text("/old/conf".to_owned())),
                ("content", Value::Text("moved\n".to_owned())),
            ],
        ),
        (
            "os:file.motd@web-01",
            &[
                ("path", Value::Text("/motd".to_owned())),
                ("content", Value::Text("old\n".to_owned())),
            ],
        ),
    ]);
    let mut state = State::new();
    state.record(
        address("os:directory.old@web-01"),
        record_of(&[("path", "/old")], &[]),
    );
    state.record(
        address("os:directory.spare@web-01"),
        record_of(&[("path", "/spare")], &[]),
    );
    state.record(
        address("os:file.conf@web-01"),
        record_of(&[("path", "/old/conf")], &["os:directory.old@web-01"]),
    );
    state.record(
        address("os:file.motd@web-01"),
        record_of(&[("path", "/motd")], &[]),
    );

    let plan = plan(&graph, &observed, &state, &os_schema(), secrets()).expect("plannable");
    assert_eq!(
        order_of(&plan),
        [
            vec![("os:directory.spare@web-01".to_owned(), ChangeKind::Delete)],
            vec![
                ("os:file.conf@web-01".to_owned(), ChangeKind::Replace),
                ("os:file.motd@web-01".to_owned(), ChangeKind::Update),
            ],
            vec![("os:directory.old@web-01".to_owned(), ChangeKind::Delete)],
        ],
        "what waits for nothing first, and the directory after the file moving out of it: {plan:?}"
    );
}

/// A moved file and the service reacting to it are planned in one run: the file is
/// replaced, then the service is roused. A reaction's record names its cause, and
/// the replacement was put after the service's change as though the service were
/// inside the file, against the declared order: a cycle, refused. Found by the
/// review of RFC 0001's fifth revision.
#[test]
#[dacc_derive::doc_anchor(
    id = "a-moved-file-and-the-service-reacting-to-it-are-planned-in-one-run"
)]
fn a_moved_file_and_the_service_reacting_to_it_are_planned_in_one_run() {
    const CONF: &str = "os:file.conf@web-01";
    const SERVICE: &str = "os:service.app@web-01";
    let graph = graph_of(vec![
        Resource::new(address(CONF))
            .with("path", "/etc/app/new.conf")
            .with("content", "c\n"),
        Resource::new(address(SERVICE))
            .with("name", "app")
            .reacting_to(address(CONF)),
    ]);
    let observed = observed_with(&[
        (
            CONF,
            &[
                ("path", Value::Text("/etc/app/old.conf".to_owned())),
                ("content", Value::Text("c\n".to_owned())),
            ],
        ),
        (SERVICE, &[("name", Value::Text("app".to_owned()))]),
    ]);
    let mut state = State::new();
    state.record(
        address(CONF),
        record_of(&[("path", "/etc/app/old.conf"), ("content", "c\n")], &[]),
    );
    state.record(address(SERVICE), record_of(&[("name", "app")], &[CONF]));

    let plan = plan(&graph, &observed, &state, &os_schema(), secrets()).expect("one run can do it");
    assert_eq!(
        order_of(&plan),
        [
            vec![(CONF.to_owned(), ChangeKind::Replace)],
            vec![(SERVICE.to_owned(), ChangeKind::React)],
        ],
        "{plan:?}"
    );
}

/// And a replacement with a resource on another machine declared after it, and
/// moving too, is planned in one run: nothing on another machine is inside it, so
/// the move does not have to go first against the declared order. Found by the
/// review of RFC 0001's fifth revision.
#[test]
#[dacc_derive::doc_anchor(
    id = "a-replacement-and-a-move-after-it-on-another-machine-are-planned-in-one-run"
)]
fn a_replacement_and_a_move_after_it_on_another_machine_are_planned_in_one_run() {
    const X: &str = "os:file.x@web-01";
    const Y: &str = "os:file.y@web-02";
    let graph = graph_of(vec![
        Resource::new(address(X))
            .with("path", "/b")
            .with("content", "x\n"),
        Resource::new(address(Y))
            .with("path", "/z")
            .with("content", "y\n")
            .after(address(X)),
    ]);
    let observed = observed_with(&[
        (
            X,
            &[
                ("path", Value::Text("/a".to_owned())),
                ("content", Value::Text("x\n".to_owned())),
            ],
        ),
        (
            Y,
            &[
                ("path", Value::Text("/y".to_owned())),
                ("content", Value::Text("y\n".to_owned())),
            ],
        ),
    ]);
    let mut state = State::new();
    state.record(
        address(X),
        record_of(&[("path", "/a"), ("content", "x\n")], &[]),
    );
    state.record(
        address(Y),
        record_of(&[("path", "/y"), ("content", "y\n")], &[X]),
    );

    let plan = plan(&graph, &observed, &state, &os_schema(), secrets()).expect("one run can do it");
    assert_eq!(
        order_of(&plan),
        [
            vec![(X.to_owned(), ChangeKind::Replace)],
            vec![(Y.to_owned(), ChangeKind::Replace)],
        ],
        "{plan:?}"
    );
}

/// A removal nothing moves out of still comes first. A service's record naming the
/// old service it came after put the old one's removal after the new one's update,
/// where the new one could find what it needs still held. Found by the review of
/// RFC 0001's fifth revision.
#[test]
#[dacc_derive::doc_anchor(id = "a-removal-nothing-moves-out-of-still-comes-first")]
fn a_removal_nothing_moves_out_of_still_comes_first() {
    const OLD: &str = "os:service.nginx@web-01";
    const NEW: &str = "os:service.caddy@web-01";
    let graph = graph_of(vec![
        Resource::new(address(NEW))
            .with("name", "caddy")
            .with("enabled", "yes"),
    ]);
    let observed = observed_with(&[(
        NEW,
        &[
            ("name", Value::Text("caddy".to_owned())),
            ("enabled", Value::Text("no".to_owned())),
        ],
    )]);
    let mut state = State::new();
    state.record(address(OLD), record_of(&[("name", "nginx")], &[]));
    state.record(
        address(NEW),
        record_of(&[("name", "caddy"), ("enabled", "no")], &[OLD]),
    );

    let plan = plan(&graph, &observed, &state, &os_schema(), secrets()).expect("plannable");
    assert_eq!(
        order_of(&plan),
        [
            vec![(OLD.to_owned(), ChangeKind::Delete)],
            vec![(NEW.to_owned(), ChangeKind::Update)],
        ],
        "{plan:?}"
    );
}

/// A declared resource naming what a removal takes away moves after it, and the
/// removal moves after the change of what state recorded after it: one order keeps
/// both, where the removal used to be refused for having to come between them.
#[test]
#[dacc_derive::doc_anchor(id = "a-resource-naming-what-a-removal-takes-moves-after-it")]
fn a_resource_naming_what_a_removal_takes_moves_after_it() {
    let os = Provider::new("os").unwrap();
    let schema = Schema::new()
        .identity(os.clone(), Kind::new("file").unwrap(), "path", "path")
        .identity(os, Kind::new("directory").unwrap(), "path", "path");
    let graph = graph_of(vec![
        Resource::new(address("os:file.conf@web-01"))
            .with("path", "/srv/app.conf")
            .with("content", "moved\n"),
        Resource::new(address("os:file.app@web-01"))
            .with("path", "/srv/app")
            .with("content", "a file now\n"),
    ]);
    let observed = observed_with(&[(
        "os:file.conf@web-01",
        &[
            ("path", Value::Text("/srv/app.conf".to_owned())),
            ("content", Value::Text("where it was\n".to_owned())),
        ],
    )]);
    let mut state = State::new();
    state.record(
        address("os:directory.app@web-01"),
        Record::new(
            Recorded::all(
                [("path".to_owned(), Value::Text("/srv/app".to_owned()))]
                    .into_iter()
                    .collect::<Attributes>(),
            ),
            BTreeSet::new(),
        ),
    );
    state.record(
        address("os:file.conf@web-01"),
        Record::new(
            BTreeMap::new(),
            [address("os:directory.app@web-01")].into_iter().collect(),
        ),
    );

    let plan = plan(&graph, &observed, &state, &schema, secrets()).expect("plannable");
    let order: Vec<Vec<(String, ChangeKind)>> = plan
        .stages()
        .iter()
        .map(|stage| {
            stage
                .changes()
                .iter()
                .map(|change| (change.address().to_string(), change.kind()))
                .collect()
        })
        .collect();
    assert_eq!(
        order,
        [
            vec![("os:file.conf@web-01".to_owned(), ChangeKind::Update)],
            vec![("os:directory.app@web-01".to_owned(), ChangeKind::Delete)],
            vec![("os:file.app@web-01".to_owned(), ChangeKind::Create)],
        ],
        "{plan:?}"
    );
}

/// An order no run can follow is refused, naming each step round it. A file
/// recorded inside a directory whose declaration goes is declared at the
/// directory's own path: the directory is removed after the file changes, and the
/// file is made after the directory is removed. Found by the review of RFC 0001's
/// fourth revision, where the refusal named the file as both sides.
#[test]
#[dacc_derive::doc_anchor(id = "an-order-no-run-can-follow-is-refused-step-by-step")]
fn an_order_no_run_can_follow_is_refused_step_by_step() {
    let os = Provider::new("os").unwrap();
    let schema = Schema::new()
        .identity(os.clone(), Kind::new("file").unwrap(), "path", "path")
        .identity(os, Kind::new("directory").unwrap(), "path", "path");
    let graph = graph_of(vec![
        Resource::new(address("os:file.conf@web-01"))
            .with("path", "/srv/app")
            .with("content", "x\n"),
    ]);
    let observed = observed_with(&[(
        "os:file.conf@web-01",
        &[
            ("path", Value::Text("/srv/app/app.conf".to_owned())),
            ("content", Value::Text("x\n".to_owned())),
        ],
    )]);
    let path = |value: &str| {
        Recorded::all(
            [("path".to_owned(), Value::Text(value.to_owned()))]
                .into_iter()
                .collect::<Attributes>(),
        )
    };
    let mut state = State::new();
    state.record(
        address("os:directory.app@web-01"),
        Record::new(path("/srv/app"), BTreeSet::new()),
    );
    state.record(
        address("os:file.conf@web-01"),
        Record::new(
            path("/srv/app/app.conf"),
            [address("os:directory.app@web-01")].into_iter().collect(),
        ),
    );

    let refused = plan(&graph, &observed, &state, &schema, secrets()).unwrap_err();
    let PlanError::Unorderable { cycle } = &refused else {
        panic!("{refused}")
    };
    assert_eq!(
        cycle,
        &[
            Step {
                later: address("os:directory.app@web-01"),
                earlier: address("os:file.conf@web-01"),
                because: Because::RemovedAfterChange,
            },
            Step {
                later: address("os:file.conf@web-01"),
                earlier: address("os:directory.app@web-01"),
                because: Because::MadeAfterRemoval,
            },
        ],
        "{refused}"
    );
}

/// The same resource with nothing recording it is adopted: nothing is done to it,
/// and it is written down, so that taking its declaration away later removes it.
/// Planned as nothing, it never was. Decision 0042.
#[test]
#[dacc_derive::doc_anchor(id = "a-resource-that-already-matches-and-is-not-recorded-is-adopted")]
fn a_resource_that_already_matches_and_is_not_recorded_is_adopted() {
    let graph = graph_of(vec![
        Resource::new(address("yandex:instance.db")).with("cores", 8),
    ]);
    let observed = observed_with(&[("yandex:instance.db", &[("cores", Value::Integer(8))])]);

    let change = only_change(&graph, &observed, &Schema::new());
    assert_eq!(change.kind(), ChangeKind::Adopt);
    assert!(change.deltas().is_empty(), "nothing differs");
    assert!(
        change.premise().is_empty(),
        "and nothing is sent to be checked"
    );
    assert_eq!(
        change.desired(),
        [("cores".to_owned(), Confidence::Known(Value::Integer(8)))],
        "what it was declared as is carried, so a secret among it is known to be one"
    );
    assert!(change.kind().is_reversible(), "nothing is destroyed");
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
    assert_eq!(
        change.deltas()[0].before(),
        Some(&Recorded::Plain(Value::Integer(4)))
    );
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

#[test]
fn a_shrink_of_a_monotonic_attribute_is_refused_naming_it() {
    let graph = graph_of(vec![
        Resource::new(address("yandex:instance.db")).with("size", 8),
    ]);
    let observed = observed_with(&[("yandex:instance.db", &[("size", Value::Integer(10))])]);
    let schema = Schema::new().monotonic(
        Provider::new("yandex").unwrap(),
        Kind::new("instance").unwrap(),
        "size",
    );

    let refused = plan(&graph, &observed, &State::new(), &schema, secrets())
        .expect_err("a shrink is refused rather than planned");
    assert!(
        refused.to_string().contains("size"),
        "the refusal names the attribute: {refused}"
    );
}

#[test]
fn a_stop_to_change_attribute_is_marked_for_the_stopped_sequence() {
    let graph = graph_of(vec![
        Resource::new(address("yandex:instance.db")).with("cores", 8),
    ]);
    let observed = observed_with(&[("yandex:instance.db", &[("cores", Value::Integer(4))])]);
    let schema = Schema::new().stop_to_change(
        Provider::new("yandex").unwrap(),
        Kind::new("instance").unwrap(),
        "cores",
    );

    let change = only_change(&graph, &observed, &schema);
    assert_eq!(change.kind(), ChangeKind::Update);
    assert_eq!(change.deltas()[0].kind(), DeltaKind::StopThenChange);
}

#[test]
fn a_grow_of_a_monotonic_attribute_is_a_normal_change() {
    let graph = graph_of(vec![
        Resource::new(address("yandex:instance.db")).with("size", 12),
    ]);
    let observed = observed_with(&[("yandex:instance.db", &[("size", Value::Integer(10))])]);
    let schema = Schema::new().monotonic(
        Provider::new("yandex").unwrap(),
        Kind::new("instance").unwrap(),
        "size",
    );

    let change = only_change(&graph, &observed, &schema);
    assert_eq!(change.kind(), ChangeKind::Update);
    assert_eq!(change.deltas()[0].kind(), DeltaKind::InPlace);
}

/// A reference is unresolved in the text, not in the world. If the resource it
/// reads from already exists, the value is known now — and without that nothing
/// reading another resource would ever converge.
#[test]
#[dacc_derive::doc_anchor(id = "a-reference-to-something-that-exists-is-already-known")]
fn a_reference_to_something_that_exists_is_already_known() {
    let graph = graph_of(vec![
        Resource::new(address("yandex:instance.db")),
        Resource::new(address("yandex:instance.web")).with(
            "upstream",
            Reference::to(address("yandex:instance.db"), "ip"),
        ),
    ]);

    let unknown = plan(
        &graph,
        &Observed::new(),
        &State::new(),
        &Schema::new(),
        secrets(),
    )
    .expect("plannable");
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
    let settled = plan(
        &graph,
        &observed,
        &recorded(&graph, &["yandex:instance.db", "yandex:instance.web"]),
        &Schema::new(),
        secrets(),
    )
    .expect("plannable");
    assert!(
        settled.is_empty(),
        "once the source exists, the reader matches: {settled:?}"
    );
}

/// The property this tool exists to have. Applying a plan and planning again
/// must produce nothing at all.
#[test]
#[dacc_derive::doc_anchor(id = "planning-after-applying-produces-an-empty-plan")]
fn planning_after_applying_produces_an_empty_plan() {
    let graph = graph_of(vec![
        Resource::new(address("yandex:instance.db")).with("cores", 8),
        Resource::new(address("os:package.nginx@web-01")).with("state", "present"),
        Resource::new(address("os:file.nginx.conf@web-01"))
            .with("listen", Reference::to(address("yandex:instance.db"), "ip")),
    ]);

    let first = plan(
        &graph,
        &Observed::new(),
        &State::new(),
        &Schema::new(),
        secrets(),
    )
    .expect("plannable");
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

    // And the state it left behind.
    let left = recorded(
        &graph,
        &[
            "yandex:instance.db",
            "os:package.nginx@web-01",
            "os:file.nginx.conf@web-01",
        ],
    );
    let second = plan(&graph, &applied, &left, &Schema::new(), secrets()).expect("plannable");
    assert!(
        second.is_empty(),
        "the second run must change nothing: {second:?}"
    );
}

/// A reference to an attribute changed in the same plan resolves to the new
/// value, so the reader is updated in that one plan and the run after applying
/// is empty.
#[test]
#[dacc_derive::doc_anchor(
    id = "a-reference-to-an-attribute-changed-in-the-same-plan-resolves-to-the-new-value"
)]
fn a_reference_to_an_attribute_changed_in_the_same_plan_resolves_to_the_new_value() {
    let graph = graph_of(vec![
        Resource::new(address("yandex:instance.db")).with("cores", 8),
        Resource::new(address("yandex:instance.web")).with(
            "upstream",
            Reference::to(address("yandex:instance.db"), "cores"),
        ),
    ]);

    // The world before the plan: four cores, and the reader holds that.
    let observed = observed_with(&[
        ("yandex:instance.db", &[("cores", Value::Integer(4))]),
        ("yandex:instance.web", &[("upstream", Value::Integer(4))]),
    ]);
    let state = recorded(&graph, &["yandex:instance.db", "yandex:instance.web"]);

    let first = plan(&graph, &observed, &state, &Schema::new(), secrets()).expect("plannable");

    let changed = first
        .changes()
        .map(|change| change.address().to_string())
        .collect::<Vec<_>>();
    assert!(
        changed.contains(&"yandex:instance.web".to_owned()),
        "the reader is updated in the same plan that changes the source: {first:?}"
    );

    // What the world looks like once that plan has been carried out.
    let applied = observed_with(&[
        ("yandex:instance.db", &[("cores", Value::Integer(8))]),
        ("yandex:instance.web", &[("upstream", Value::Integer(8))]),
    ]);
    let second = plan(&graph, &applied, &state, &Schema::new(), secrets()).expect("plannable");
    assert!(
        second.is_empty(),
        "the run after applying is empty: {second:?}"
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

    let plan = plan(&graph, &observed, &State::new(), &schema, secrets()).expect("plannable");
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
#[dacc_derive::doc_anchor(id = "stages-with-nothing-to-do-are-left-out-but-order-is-kept")]
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

    let plan = plan(
        &graph,
        &observed,
        &recorded(&graph, &["yandex:network.main"]),
        &Schema::new(),
        secrets(),
    )
    .expect("plannable");
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
#[dacc_derive::doc_anchor(id = "a-graph-that-cannot-be-ordered-is-not-planned-at-all")]
fn a_graph_that_cannot_be_ordered_is_not_planned_at_all() {
    let graph = graph_of(vec![
        Resource::new(address("os:file.a@web-01")).after(address("os:file.b@web-01")),
        Resource::new(address("os:file.b@web-01")).after(address("os:file.a@web-01")),
    ]);

    assert!(
        plan(
            &graph,
            &Observed::new(),
            &State::new(),
            &Schema::new(),
            secrets()
        )
        .is_err()
    );
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

    let radius = plan(&graph, &observed, &State::new(), &Schema::new(), secrets())
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

    let work = plan(
        &graph,
        &Observed::new(),
        &State::new(),
        &Schema::new(),
        secrets(),
    )
    .expect("plannable");
    assert!(!work.is_empty(), "there is a resource to create");
    assert_eq!(work.changes().count(), 1);

    let settled = observed_with(&[("yandex:instance.db", &[("cores", Value::Integer(8))])]);
    let nothing = plan(
        &graph,
        &settled,
        &recorded(&graph, &["yandex:instance.db"]),
        &Schema::new(),
        secrets(),
    )
    .expect("plannable");
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
        Record::new(Recorded::all(Attributes::new()), BTreeSet::new()),
    );

    let plan =
        plan(&graph, &Observed::new(), &state, &Schema::new(), secrets()).expect("plannable");
    let radius = plan.blast_radius();

    assert_eq!(radius.creates(), 2);
    assert_eq!(radius.deletes(), 1);
    assert_eq!(
        radius.total(),
        3,
        "two made and one removed is three changes"
    );
}

/// A plan is what an agent decides from, so it has to survive being written out
/// and read back exactly — including the values that are not known yet.
#[test]
#[dacc_derive::doc_anchor(id = "a-plan-survives-a-trip-through-json")]
fn a_plan_survives_a_trip_through_json() {
    let graph = graph_of(vec![
        Resource::new(address("yandex:instance.db"))
            .with("zone", "e")
            .with("cores", 8),
        Resource::new(address("os:file.conf@web-01"))
            .with("path", "/etc/app.conf")
            .with("listen", Reference::to(address("yandex:instance.db"), "ip")),
    ]);
    let observed = observed_with(&[(
        "yandex:instance.db",
        &[
            ("zone", Value::Text("a".to_owned())),
            ("cores", Value::Integer(8)),
        ],
    )]);
    let schema = Schema::new().immutable(
        Provider::new("yandex").unwrap(),
        Kind::new("instance").unwrap(),
        "zone",
    );

    let written = plan(&graph, &observed, &State::new(), &schema, secrets()).expect("plannable");
    let text = serde_json::to_string(&written).expect("writes");
    let read: nerpa_core::Plan = serde_json::from_str(&text).expect("reads");

    assert_eq!(
        read, written,
        "everything came back, including what is not known yet"
    );
    assert!(
        text.contains("\"yandex:instance.db\""),
        "addresses travel as text: {text}"
    );
    assert!(
        text.contains("replace"),
        "and so does the kind of change: {text}"
    );
}

/// A value that cannot reach a machine in one operation is refused while
/// planning, at exactly the bound its provider declares, rather than found when
/// its batch is written, after earlier changes in the run have been made.
#[test]
#[dacc_derive::doc_anchor(id = "a-value-too-large-for-one-operation-is-refused-when-planning")]
fn a_value_too_large_for_one_operation_is_refused_when_planning() {
    let schema = Schema::new().bounded(
        Provider::new("os").unwrap(),
        Kind::new("file").unwrap(),
        "content",
        8,
    );
    let at_the_bound = graph_of(vec![
        Resource::new(address("os:file.motd@web-01")).with("content", "12345678"),
    ]);
    plan(
        &at_the_bound,
        &Observed::new(),
        &State::new(),
        &schema,
        secrets(),
    )
    .expect("eight bytes is within a bound of eight");

    let over = graph_of(vec![
        Resource::new(address("os:file.motd@web-01")).with("content", "123456789"),
    ]);
    let refusal = plan(&over, &Observed::new(), &State::new(), &schema, secrets())
        .expect_err("nine bytes is over it");
    let PlanError::TooLarge {
        attribute,
        size,
        most,
        ..
    } = refusal
    else {
        panic!("refused for the wrong reason: {refusal}")
    };
    assert_eq!((attribute.as_str(), size, most), ("content", 9, 8));
}

/// A reaction on a kind that cannot react is refused when planning as well, for a
/// graph that did not come from a configuration.
#[test]
#[dacc_derive::doc_anchor(id = "a-kind-that-cannot-react-is-refused-when-planning")]
fn a_kind_that_cannot_react_is_refused_when_planning() {
    let schema =
        Schema::new().cannot_react(Provider::new("os").unwrap(), Kind::new("package").unwrap());
    let graph = graph_of(vec![
        Resource::new(address("os:file.conf@web-01")).with("path", "/etc/app.conf"),
        Resource::new(address("os:package.app-misc/jq@web-01"))
            .with("version", "1.8.1")
            .reacting_to(address("os:file.conf@web-01")),
    ]);
    let refusal = plan(&graph, &Observed::new(), &State::new(), &schema, secrets())
        .expect_err("a package cannot react");
    assert!(
        matches!(&refusal, PlanError::CannotReact { address: refused, .. } if *refused == address("os:package.app-misc/jq@web-01")),
        "{refusal}"
    );
}

/// One thing on a machine is declared once. A file and a directory at one path,
/// however the path is spelled, are the same thing there; the same path on
/// another machine is not.
#[test]
#[dacc_derive::doc_anchor(id = "two-resources-naming-one-thing-on-a-machine-are-refused")]
fn two_resources_naming_one_thing_on_a_machine_are_refused() {
    let os = Provider::new("os").unwrap();
    let schema = Schema::new()
        .identity(os.clone(), Kind::new("file").unwrap(), "path", "path")
        .identity(os, Kind::new("directory").unwrap(), "path", "path");

    let apart = graph_of(vec![
        Resource::new(address("os:file.motd@web-01")).with("path", "/etc/motd"),
        Resource::new(address("os:file.motd@web-02")).with("path", "/etc/motd"),
    ]);
    plan(&apart, &Observed::new(), &State::new(), &schema, secrets())
        .expect("the same path on two machines is two things");

    let together = graph_of(vec![
        Resource::new(address("os:file.one@web-01")).with("path", "/etc/app/conf"),
        Resource::new(address("os:directory.two@web-01")).with("path", "/etc//app/./conf/"),
    ]);
    let refusal = plan(
        &together,
        &Observed::new(),
        &State::new(),
        &schema,
        secrets(),
    )
    .expect_err("one path, two declarations");
    let PlanError::SameThing {
        first,
        second,
        namespace,
        ..
    } = refusal
    else {
        panic!("refused for the wrong reason: {refusal}")
    };
    let named = [first.to_string(), second.to_string()];
    assert!(
        named.contains(&"os:file.one@web-01".to_owned())
            && named.contains(&"os:directory.two@web-01".to_owned()),
        "{named:?}"
    );
    assert_eq!(namespace, "path");
}
