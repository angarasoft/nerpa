//! An instance converges against the in-memory fake: create, read back, and a
//! second run has nothing to do.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use nerpa_core::address::address;
use nerpa_core::provider::{Machines, Provider, Providers};
use nerpa_core::run::{apply, observe};
use nerpa_core::schema::Schema;
use nerpa_core::secret::{Key, Remembered, Secrets};
use nerpa_core::state::State;
use nerpa_core::{ChangeKind, Graph, Resource};
use nerpa_provider_yandex::Yandex;

fn plan(
    graph: &Graph,
    observed: &nerpa_core::Observed,
    state: &State,
    schema: &Schema,
    secrets: Secrets<'_>,
) -> Result<nerpa_core::Plan, nerpa_core::planner::PlanError> {
    nerpa_core::planner::plan(graph, observed, state, schema, secrets).map(|planned| {
        planned.attributed_to(nerpa_core::Actor::new("tester", "run", None).expect("an actor"))
    })
}

static DIGEST_KEY: Key = Key::from_bytes([5; 32]);
static NO_STORE: Remembered = Remembered::EMPTY;

fn secrets() -> Secrets<'static> {
    Secrets::new(&DIGEST_KEY, &NO_STORE)
}

#[test]
fn an_instance_converges_against_the_fake() {
    let instance = address("yandex:instance.db");
    let mut graph = Graph::new();
    graph
        .insert(
            Resource::new(instance.clone())
                .with("zone_id", "ru-central1-b")
                .with("platform_id", "standard-v3")
                .with("cores", 2)
                .with("memory_gb", 2),
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

    let observed = observe(&graph, &providers, &machines, &state).expect("observed");
    let second =
        plan(&graph, &observed, &state, &providers.schema(), secrets()).expect("plannable");
    assert_eq!(
        second.changes().count(),
        0,
        "a second run has nothing to do"
    );
}
