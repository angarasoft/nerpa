//! A reaction is an edge, and the plan says it before anything happens.
//!
//! Ansible finds handlers by name in a global table, runs them at the end of the
//! play, and tells nobody beforehand. Each of those is a failure people work
//! around rather than a design; decision 0026 makes a reaction an ordinary edge
//! so that none of the three is reachable.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::{BTreeMap, BTreeSet};

use nerpa_core::address::address;
use nerpa_core::planner::plan;
use nerpa_core::schema::Schema;
use nerpa_core::secret::{Key, Remembered, Secrets};
use nerpa_core::state::{Record, State};
use nerpa_core::store;
use nerpa_core::{ChangeKind, Graph, Observed, Resource, Value};

static DIGEST_KEY: Key = Key::from_bytes([9; 32]);
static NO_STORE: Remembered = Remembered::EMPTY;

fn secrets() -> Secrets<'static> {
    Secrets::new(&DIGEST_KEY, &NO_STORE)
}

const CONF: &str = "os:file.nginx-conf@web-01";
const SERVICE: &str = "os:service.nginx@web-01";

/// A configuration file and a service that reacts to it, both already made.
fn world(conf_says: &str) -> (Graph, Observed, State) {
    let mut graph = Graph::new();
    graph.preexisting(nerpa_core::Target::new("web-01").unwrap());
    graph
        .insert(Resource::new(address(CONF)).with("content", conf_says))
        .unwrap();
    graph
        .insert(
            Resource::new(address(SERVICE))
                .with("name", "nginx")
                .reacting_to(address(CONF)),
        )
        .unwrap();

    let mut observed = Observed::new();
    observed.record(
        address(CONF),
        [("content".to_owned(), Value::Text("as it was\n".to_owned()))]
            .into_iter()
            .collect(),
    );
    observed.record(
        address(SERVICE),
        [("name".to_owned(), Value::Text("nginx".to_owned()))]
            .into_iter()
            .collect(),
    );

    let mut state = State::new();
    for named in [CONF, SERVICE] {
        state.record(
            address(named),
            Record::new(BTreeMap::new(), BTreeSet::default()),
        );
    }
    (graph, observed, state)
}

fn planned(conf_says: &str) -> nerpa_core::Plan {
    let (graph, observed, state) = world(conf_says);
    plan(&graph, &observed, &state, &Schema::new(), secrets()).expect("plannable")
}

/// The property no predecessor has: the restart is in the plan, with its cause,
/// before anything happens.
#[test]
fn a_service_whose_file_changes_is_roused_and_the_plan_says_by_what() {
    let plan = planned("as it will be\n");

    let roused: Vec<_> = plan
        .stages()
        .iter()
        .flat_map(nerpa_core::PlanStage::changes)
        .filter(|change| change.kind() == ChangeKind::React)
        .collect();

    assert_eq!(roused.len(), 1, "the service, and only the service");
    assert_eq!(roused[0].address(), &address(SERVICE));
    assert_eq!(
        roused[0].roused_by(),
        [address(CONF)],
        "and the plan names which file did it"
    );
    assert_eq!(plan.blast_radius().reacts(), 1);
    assert_eq!(plan.blast_radius().total(), 2, "the file and the service");
}

/// Ordered, not piled at the end: the restart comes after the file it is about.
#[test]
fn a_reaction_comes_after_the_thing_that_caused_it() {
    let plan = planned("as it will be\n");

    let stage_of = |named: &str| {
        plan.stages()
            .iter()
            .position(|stage| {
                stage
                    .changes()
                    .iter()
                    .any(|change| change.address() == &address(named))
            })
            .expect("planned")
    };
    assert!(
        stage_of(CONF) < stage_of(SERVICE),
        "restarting before the file it reads would be acting on the old world"
    );
}

/// And a file that did not change rouses nothing. A reaction that fires anyway
/// is a fleet-wide restart nobody asked for.
#[test]
fn a_service_is_not_roused_when_nothing_it_reacts_to_changed() {
    let plan = planned("as it was\n");
    assert!(plan.is_empty(), "nothing to do: {plan:?}");
}

/// The property no predecessor has, and the half that makes the rest worth
/// having: a reaction that was triggered and did not complete is written down,
/// and the next plan says so.
///
/// Ansible runs handlers at the end of a play, so a failure between the write
/// and the end skips them and leaves a new file, an old process, and nothing
/// recording that fact. `--force-handlers` changes when they run rather than
/// whether the debt is remembered.
#[test]
fn a_reaction_that_did_not_finish_is_owed_and_the_next_plan_says_so() {
    let (graph, observed, mut state) = world("as it was\n");

    // A run that got as far as writing the debt down and no further.
    state.owe(&address(SERVICE), &[address(CONF)]);

    let plan = plan(&graph, &observed, &state, &Schema::new(), secrets()).expect("plannable");
    let roused: Vec<_> = plan
        .stages()
        .iter()
        .flat_map(nerpa_core::PlanStage::changes)
        .filter(|change| change.kind() == ChangeKind::React)
        .collect();

    assert_eq!(
        roused.len(),
        1,
        "the service is roused even though nothing changed this time: {plan:?}"
    );
    assert_eq!(roused[0].owed(), [address(CONF)], "and the plan says why");
    assert!(
        roused[0].roused_by().is_empty(),
        "which is a different fact from a file changing now"
    );
}

/// And a debt that was paid does not come back. State keeps a record per
/// resource, and a fresh one owes nothing — which is how paying works.
#[test]
fn a_reaction_that_finished_leaves_nothing_owed() {
    let (graph, observed, mut state) = world("as it was\n");
    state.owe(&address(SERVICE), &[address(CONF)]);

    // What `apply` writes once the reaction has been acted on.
    state.record(
        address(SERVICE),
        Record::new(BTreeMap::new(), BTreeSet::default()),
    );

    let plan = plan(&graph, &observed, &state, &Schema::new(), secrets()).expect("plannable");
    assert!(plan.is_empty(), "nothing is owed any more: {plan:?}");
}

/// A debt outlives the process that could not pay it, or it is not a debt.
#[test]
fn a_debt_survives_the_state_file() {
    let scratch = std::env::temp_dir().join(format!("nerpa-owed-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&scratch);
    let file = scratch.join("state.json");

    let (_, _, mut state) = world("as it was\n");
    state.owe(&address(SERVICE), &[address(CONF)]);

    let key = Key::from_bytes([4; 32]);
    store::save(&file, &state, &key).expect("writes");
    let (read, _) = store::load(&file).expect("reads");

    assert_eq!(
        read.get(&address(SERVICE))
            .expect("the service is remembered")
            .owed()
            .iter()
            .collect::<Vec<_>>(),
        vec![&address(CONF)],
        "a debt a run could not pay has to outlive the run"
    );
    let _ = std::fs::remove_dir_all(&scratch);
}
