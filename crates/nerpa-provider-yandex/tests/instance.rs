//! An instance converges against the in-memory fake: create, read back, and
//! a second run has nothing to do. A change to its resources stops the
//! running instance first and starts it after; the plan says the stop it
//! requires; a boot disk only grows, and a shrink of it is refused while
//! planning.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use nerpa_core::address::address;
use nerpa_core::plan::DeltaKind;
use nerpa_core::planner::PlanError;
use nerpa_core::provider::{Machines, Provider, Providers};
use nerpa_core::run::{apply, observe};
use nerpa_core::schema::Schema;
use nerpa_core::secret::{Key, Remembered, Secrets};
use nerpa_core::state::State;
use nerpa_core::{ChangeKind, Graph, Resource, Value};
use nerpa_provider_yandex::Yandex;

fn plan(
    graph: &Graph,
    observed: &nerpa_core::Observed,
    state: &State,
    schema: &Schema,
    secrets: Secrets<'_>,
) -> Result<nerpa_core::Plan, PlanError> {
    nerpa_core::planner::plan(graph, observed, state, schema, secrets).map(|planned| {
        planned.attributed_to(
            nerpa_core::Actor::new("tester", nerpa_core::Grant::default(), None).expect("an actor"),
        )
    })
}

static DIGEST_KEY: Key = Key::from_bytes([5; 32]);
static NO_STORE: Remembered = Remembered::EMPTY;

fn secrets() -> Secrets<'static> {
    Secrets::new(&DIGEST_KEY, &NO_STORE)
}

/// The declaration an instance is born from: its zone, its platform, its
/// resources, the subnet its interface lives in, and the image its boot disk
/// is made from.
fn declaration(instance: &nerpa_core::Address, memory_gb: i64, boot_disk_gb: i64) -> Graph {
    let mut graph = Graph::new();
    graph
        .insert(
            Resource::new(instance.clone())
                .with("zone_id", "ru-central1-b")
                .with("platform_id", "standard-v3")
                .with("cores", 2)
                .with("memory_gb", memory_gb)
                .with("subnet_id", "id-subnet")
                .with("boot_disk_image_id", "fd-ubuntu")
                .with("boot_disk_gb", boot_disk_gb),
        )
        .expect("a resource");
    graph
}

/// One declared world, made so.
fn settles(
    graph: &Graph,
    providers: &Providers,
    machines: &Machines,
    state: &mut State,
) -> nerpa_core::Plan {
    let observed = observe(graph, providers, machines, state).expect("observed");
    let planned = plan(graph, &observed, state, &providers.schema(), secrets()).expect("plannable");
    apply(
        &planned,
        providers,
        machines,
        &observed,
        state,
        secrets(),
        &mut |_| Ok(()),
    )
    .expect("applied");
    planned
}

#[test]
fn an_instance_converges_against_the_fake() {
    let instance = address("yandex:instance.db");
    let graph = declaration(&instance, 2, 10);

    let mut state = State::new();
    let providers = Providers::new().with(Box::new(Yandex::fake()));
    let machines = Machines::new();

    let observed = observe(&graph, &providers, &machines, &state).expect("observed");
    let first = plan(&graph, &observed, &state, &providers.schema(), secrets()).expect("plannable");
    assert_eq!(
        first.stages()[0].changes()[0].kind(),
        ChangeKind::Create,
        "a resource not out there is created"
    );

    apply(
        &first,
        &providers,
        &machines,
        &observed,
        &mut state,
        secrets(),
        &mut |_| Ok(()),
    )
    .expect("applied");
    assert!(state.get(&instance).is_some(), "the instance was recorded");
    let held = state.get(&instance).expect("recorded");
    assert_eq!(
        held.attributes()
            .get("boot_disk_image_id")
            .and_then(nerpa_core::secret::Recorded::plain),
        Some(&Value::Text("fd-ubuntu".to_owned())),
        "the image the disk was made from is reported"
    );

    let observed = observe(&graph, &providers, &machines, &state).expect("observed");
    let second =
        plan(&graph, &observed, &state, &providers.schema(), secrets()).expect("plannable");
    assert_eq!(
        second.changes().count(),
        0,
        "a second run has nothing to do"
    );
}

#[test]
fn a_resources_change_stops_the_running_instance_and_starts_it_after() {
    let instance = address("yandex:instance.db");
    let mut state = State::new();
    let providers = Providers::new().with(Box::new(Yandex::fake()));
    let machines = Machines::new();

    settles(
        &declaration(&instance, 2, 10),
        &providers,
        &machines,
        &mut state,
    );
    // The memory grows: the change reaches the instance only while it is
    // stopped, and the provider stops it, changes it and starts it again —
    // the fake cloud refuses a resources change on a running one.
    settles(
        &declaration(&instance, 4, 10),
        &providers,
        &machines,
        &mut state,
    );

    let graph = declaration(&instance, 4, 10);
    let observed = observe(&graph, &providers, &machines, &state).expect("observed");
    let third = plan(&graph, &observed, &state, &providers.schema(), secrets()).expect("plannable");
    assert_eq!(
        third.changes().count(),
        0,
        "a run after the change has nothing to do"
    );
}

#[test]
fn the_plan_shows_the_stop_a_resources_change_requires() {
    let instance = address("yandex:instance.db");
    let mut state = State::new();
    let providers = Providers::new().with(Box::new(Yandex::fake()));
    let machines = Machines::new();

    settles(
        &declaration(&instance, 2, 10),
        &providers,
        &machines,
        &mut state,
    );

    let graph = declaration(&instance, 4, 10);
    let observed = observe(&graph, &providers, &machines, &state).expect("observed");
    let second =
        plan(&graph, &observed, &state, &providers.schema(), secrets()).expect("plannable");
    let change = &second.stages()[0].changes()[0];
    assert_eq!(change.kind(), ChangeKind::Update, "the memory changed");
    let memory = change
        .deltas()
        .iter()
        .find(|delta| delta.name() == "memory_gb")
        .expect("the memory is the difference");
    assert_eq!(
        memory.kind(),
        DeltaKind::StopThenChange,
        "the plan carries the stop the change requires"
    );
}

#[test]
fn a_boot_disk_shrink_is_refused_while_planning() {
    let instance = address("yandex:instance.db");
    let mut state = State::new();
    let providers = Providers::new().with(Box::new(Yandex::fake()));
    let machines = Machines::new();

    settles(
        &declaration(&instance, 2, 10),
        &providers,
        &machines,
        &mut state,
    );

    // The declared disk is smaller than the one there: a value that only
    // grows is refused while planning, by name, rather than proposed to a
    // cloud that would refuse it too.
    let graph = declaration(&instance, 2, 5);
    let observed = observe(&graph, &providers, &machines, &state).expect("observed");
    let error = plan(&graph, &observed, &state, &providers.schema(), secrets())
        .expect_err("a shrink is refused");
    assert!(
        matches!(&error, PlanError::ShrinkRefused { attribute, .. } if attribute == "boot_disk_gb"),
        "the refusal names the boot disk: {error}"
    );
}

#[test]
fn a_boot_disk_grows_where_the_instance_stands() {
    let instance = address("yandex:instance.db");
    let mut state = State::new();
    let providers = Providers::new().with(Box::new(Yandex::fake()));
    let machines = Machines::new();

    settles(
        &declaration(&instance, 2, 10),
        &providers,
        &machines,
        &mut state,
    );
    settles(
        &declaration(&instance, 2, 11),
        &providers,
        &machines,
        &mut state,
    );

    let graph = declaration(&instance, 2, 11);
    let observed = observe(&graph, &providers, &machines, &state).expect("observed");
    let third = plan(&graph, &observed, &state, &providers.schema(), secrets()).expect("plannable");
    assert_eq!(
        third.changes().count(),
        0,
        "a run after the grow has nothing to do"
    );
    let held = state.get(&instance).expect("recorded");
    assert_eq!(
        held.attributes()
            .get("boot_disk_gb")
            .and_then(nerpa_core::secret::Recorded::plain),
        Some(&Value::Integer(11)),
        "the grown size is what a run after sees"
    );
}

#[test]
fn a_network_converges_against_the_fake() {
    let network = address("yandex:network.main");
    let mut graph = Graph::new();
    graph
        .insert(Resource::new(network.clone()))
        .expect("a resource");

    let mut state = State::new();
    let providers = Providers::new().with(Box::new(Yandex::fake()));
    let machines = Machines::new();

    let observed = observe(&graph, &providers, &machines, &state).expect("observed");
    let first = plan(&graph, &observed, &state, &providers.schema(), secrets()).expect("plannable");
    assert_eq!(
        first.stages()[0].changes()[0].kind(),
        ChangeKind::Create,
        "a network not out there is created"
    );

    apply(
        &first,
        &providers,
        &machines,
        &observed,
        &mut state,
        secrets(),
        &mut |_| Ok(()),
    )
    .expect("applied");
    assert!(state.get(&network).is_some(), "the network was recorded");

    let observed = observe(&graph, &providers, &machines, &state).expect("observed");
    let second =
        plan(&graph, &observed, &state, &providers.schema(), secrets()).expect("plannable");
    assert_eq!(
        second.changes().count(),
        0,
        "a second run has nothing to do"
    );
}

#[test]
fn a_subnet_converges_against_the_fake() {
    let subnet = address("yandex:subnet.db");
    let mut graph = Graph::new();
    graph
        .insert(
            Resource::new(subnet.clone())
                .with("network_id", "id-main")
                .with("zone_id", "ru-central1-b")
                .with(
                    "v4_cidr_blocks",
                    Value::List(vec![Value::Text("10.0.0.0/24".to_owned())]),
                ),
        )
        .expect("a resource");

    let mut state = State::new();
    let providers = Providers::new().with(Box::new(Yandex::fake()));
    let machines = Machines::new();

    let observed = observe(&graph, &providers, &machines, &state).expect("observed");
    let first = plan(&graph, &observed, &state, &providers.schema(), secrets()).expect("plannable");
    assert_eq!(
        first.stages()[0].changes()[0].kind(),
        ChangeKind::Create,
        "a subnet not out there is created"
    );

    apply(
        &first,
        &providers,
        &machines,
        &observed,
        &mut state,
        secrets(),
        &mut |_| Ok(()),
    )
    .expect("applied");
    assert!(state.get(&subnet).is_some(), "the subnet was recorded");

    let observed = observe(&graph, &providers, &machines, &state).expect("observed");
    let second =
        plan(&graph, &observed, &state, &providers.schema(), secrets()).expect("plannable");
    assert_eq!(
        second.changes().count(),
        0,
        "a second run has nothing to do"
    );
}
