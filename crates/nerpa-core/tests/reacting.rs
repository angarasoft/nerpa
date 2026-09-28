//! A reaction is an edge, and the plan says it before anything happens.
//!
//! Ansible finds handlers by name in a global table, runs them at the end of the
//! play, and tells nobody beforehand. Each of those is a failure people work
//! around rather than a design; decision 0026 makes a reaction an ordinary edge
//! so that none of the three is reachable.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::{BTreeMap, BTreeSet};

use nerpa_core::Address;
use nerpa_core::address::address;
use nerpa_core::planner::plan;
use nerpa_core::provider::{Machines, Provider, ProviderError, Providers, Reach, Read, Reading};
use nerpa_core::run::apply;
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

    // As the run that made them left it: the service came after its file.
    let mut state = State::new();
    state.record(
        address(CONF),
        Record::new(BTreeMap::new(), BTreeSet::default()),
    );
    state.record(
        address(SERVICE),
        Record::new(BTreeMap::new(), [address(CONF)].into_iter().collect()),
    );
    (graph, observed, state)
}

fn planned(conf_says: &str) -> nerpa_core::Plan {
    let (graph, observed, state) = world(conf_says);
    plan(&graph, &observed, &state, &Schema::new(), secrets()).expect("plannable")
}

/// The property no predecessor has: the restart is in the plan, with its cause,
/// before anything happens.
#[test]
#[dacc_derive::doc_anchor(id = "a-service-whose-file-changes-is-roused-and-the-plan-says-by-what")]
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
#[dacc_derive::doc_anchor(id = "a-reaction-that-did-not-finish-is-owed-and-the-next-plan-says-so")]
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
        Record::new(BTreeMap::new(), [address(CONF)].into_iter().collect()),
    );

    let plan = plan(&graph, &observed, &state, &Schema::new(), secrets()).expect("plannable");
    assert!(plan.is_empty(), "nothing is owed any more: {plan:?}");
}

/// A debt outlives the process that could not pay it, or it is not a debt.
#[test]
#[dacc_derive::doc_anchor(id = "a-debt-survives-the-state-file")]
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

/// A file that is adopted or reordered did not change, and rouses nothing. Only
/// state is written for it; a service reacting to it would otherwise be restarted
/// on every machine a first run adopts, for a file nobody touched.
#[test]
#[dacc_derive::doc_anchor(id = "a-file-adopted-or-reordered-rouses-nothing")]
fn a_file_adopted_or_reordered_rouses_nothing() {
    let (graph, observed, _) = world("as it was\n");
    let adopted =
        plan(&graph, &observed, &State::new(), &Schema::new(), secrets()).expect("plannable");
    assert_eq!(adopted.blast_radius().adopts(), 2, "{adopted:?}");
    assert_eq!(
        adopted.blast_radius().reacts(),
        0,
        "nothing changed to react to: {adopted:?}"
    );

    // Recorded with an edge it no longer has, so the file is reordered.
    let (_, _, mut state) = world("as it was\n");
    state.record(
        address(CONF),
        Record::new(
            BTreeMap::new(),
            [address("os:file.gone@web-01")].into_iter().collect(),
        ),
    );
    let reordered = plan(&graph, &observed, &state, &Schema::new(), secrets()).expect("plannable");
    assert_eq!(reordered.blast_radius().reorders(), 1, "{reordered:?}");
    assert_eq!(
        reordered.blast_radius().reacts(),
        0,
        "nothing changed to react to: {reordered:?}"
    );
}

/// Writes what it is given and reports it back, except for the one resource that
/// stands for the run stopping: a machine gone, a Ctrl-C, anything that comes
/// between a cause and the reaction to it.
#[derive(Debug)]
struct StopsAt(&'static str);

impl Provider for StopsAt {
    fn name(&self) -> &nerpa_core::Provider {
        static NAME: std::sync::OnceLock<nerpa_core::Provider> = std::sync::OnceLock::new();
        NAME.get_or_init(|| nerpa_core::Provider::new("os").unwrap())
    }

    fn schema(&self) -> Schema {
        Schema::new()
    }

    fn read(
        &self,
        _batch: &[Reading],
        _facts: nerpa_core::observed::Facts<'_>,
        _reach: &dyn Reach,
    ) -> Result<Read, ProviderError> {
        Ok(Read::new())
    }

    fn prepare(
        &self,
        change: &nerpa_core::provider::Prepared<'_>,
    ) -> Result<nerpa_core::provider::Planned, ProviderError> {
        if change.address.to_string() == self.0 {
            return Err(ProviderError::World {
                address: change.address.clone(),
                problem: "the machine went away".to_owned(),
            });
        }
        Ok(nerpa_core::provider::Planned::default())
    }

    fn conclude(
        &self,
        change: &nerpa_core::provider::Concluding<'_>,
        _became: &[nerpa_core::provider::Answered],
    ) -> Result<nerpa_core::provider::Concluded, ProviderError> {
        Ok(nerpa_core::provider::Concluded::Became(
            change.plain.clone(),
        ))
    }

    fn afterwards(
        &self,
        _address: &Address,
        _kind: nerpa_core::provider::Sending,
    ) -> nerpa_core::provider::Afterwards {
        nerpa_core::provider::Afterwards::Read
    }

    fn fake() -> Self {
        Self("")
    }
}

/// Reaches nothing, because this provider never asks.
#[derive(Debug)]
struct Nowhere;

impl Reach for Nowhere {
    fn ask(&self, _ask: &nerpa_proto::Ask) -> Result<nerpa_proto::Answer, ProviderError> {
        panic!("this provider does not speak to a machine")
    }
}

/// What stands between the file and the service in the run that stops.
const BLOCKER: &str = "os:file.blocker@web-01";

/// Plans the file changing, something in the way, and the service after it;
/// applies that, stopping at what is in the way; and plans again against the world
/// the run left behind: the file written, and nothing after it.
fn replanned_after_stopping_between(mut state: State) -> nerpa_core::Plan {
    let mut graph = Graph::new();
    graph.preexisting(nerpa_core::Target::new("web-01").unwrap());
    graph
        .insert(Resource::new(address(CONF)).with("content", "as it will be\n"))
        .unwrap();
    graph
        .insert(
            Resource::new(address(BLOCKER))
                .with("content", "in the way\n")
                .after(address(CONF)),
        )
        .unwrap();
    graph
        .insert(
            Resource::new(address(SERVICE))
                .with("name", "nginx")
                .reacting_to(address(CONF))
                .after(address(BLOCKER)),
        )
        .unwrap();

    let (_, observed, _) = world("as it was\n");
    let providers = Providers::new().with(Box::new(StopsAt(BLOCKER)));
    let first = plan(&graph, &observed, &state, &Schema::new(), secrets()).expect("plannable");
    assert_eq!(
        first.stages().len(),
        3,
        "the file, what is in the way, the service: {first:?}"
    );

    let outcome = apply(
        &first,
        &providers,
        &Machines::new().everywhere(Box::new(Nowhere)),
        &observed,
        &mut state,
        secrets(),
        &mut |_: &State| Ok(()),
    );
    assert!(
        outcome.is_err(),
        "the run stopped before the service's stage"
    );

    let mut now = observed.clone();
    now.record(
        address(CONF),
        [(
            "content".to_owned(),
            Value::Text("as it will be\n".to_owned()),
        )]
        .into_iter()
        .collect(),
    );
    plan(&graph, &now, &state, &Schema::new(), secrets()).expect("plannable")
}

/// A reaction is owed from before its cause is sent, not from when its own stage
/// comes. A run that stopped in between used to leave the new file recorded and
/// the restart forgotten: the next plan found the file matching, nothing changing,
/// and nothing owed. Found by the review of RFC 0001.
#[test]
#[dacc_derive::doc_anchor(
    id = "a-run-that-stops-between-a-cause-and-its-reaction-leaves-the-reaction-owed"
)]
fn a_run_that_stops_between_a_cause_and_its_reaction_leaves_the_reaction_owed() {
    let (_, _, state) = world("as it was\n");
    let next = replanned_after_stopping_between(state);

    let service = next
        .changes()
        .find(|change| change.address() == &address(SERVICE))
        .expect("the service is in the plan");
    assert_eq!(service.kind(), ChangeKind::React, "{next:?}");
    assert_eq!(
        service.owed(),
        [address(CONF)],
        "owed since the run that stopped"
    );
}

/// And a service that state does not record yet — found as declared, and roused
/// in the plan that would otherwise have adopted it — is recorded from what was
/// read, so that the debt has somewhere to be written.
#[test]
#[dacc_derive::doc_anchor(id = "a-reactor-nothing-recorded-yet-is-owed-its-reaction-all-the-same")]
fn a_reactor_nothing_recorded_yet_is_owed_its_reaction_all_the_same() {
    let mut state = State::new();
    state.record(
        address(CONF),
        Record::new(BTreeMap::new(), BTreeSet::default()),
    );
    let next = replanned_after_stopping_between(state);

    let service = next
        .changes()
        .find(|change| change.address() == &address(SERVICE))
        .expect("the service is in the plan");
    assert_eq!(service.kind(), ChangeKind::React, "{next:?}");
    assert_eq!(
        service.owed(),
        [address(CONF)],
        "owed since the run that stopped"
    );
}

/// A service on a second machine, which reacts to the file on the first.
const FAR_SERVICE: &str = "os:service.nginx@web-02";

/// The file changing on a machine that answers, recorded, and a service reacting
/// to it declared on another.
fn far_world() -> (Graph, Observed, State) {
    let mut graph = Graph::new();
    for node in ["web-01", "web-02"] {
        graph.preexisting(nerpa_core::Target::new(node).unwrap());
    }
    graph
        .insert(Resource::new(address(CONF)).with("content", "as it will be\n"))
        .unwrap();
    graph
        .insert(
            Resource::new(address(FAR_SERVICE))
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
    observed.unreachable(nerpa_core::Target::new("web-02").unwrap());

    let mut state = State::new();
    state.record(
        address(CONF),
        Record::new(BTreeMap::new(), BTreeSet::default()),
    );
    (graph, observed, state)
}

/// A service on a machine that did not answer, reacting to a file on one that
/// did, is owed the reaction on its record before the file is sent. Nothing was
/// owed: when the machine came back the file matched, nothing was changing, and
/// the service kept the old file. Every reaction test ran on one machine. Found by
/// the narrow review of RFC 0001.
#[test]
#[dacc_derive::doc_anchor(id = "a-reaction-on-a-machine-that-did-not-answer-is-owed-on-its-record")]
fn a_reaction_on_a_machine_that_did_not_answer_is_owed_on_its_record() {
    let (graph, observed, mut state) = far_world();
    state.record(
        address(FAR_SERVICE),
        Record::new(BTreeMap::new(), [address(CONF)].into_iter().collect()),
    );

    let first = plan(&graph, &observed, &state, &Schema::new(), secrets()).expect("plannable");
    let file = first
        .changes()
        .find(|change| change.address() == &address(CONF))
        .expect("the file is sent");
    assert_eq!(file.owed_by(), [address(FAR_SERVICE)], "{first:?}");

    apply(
        &first,
        &Providers::new().with(Box::new(StopsAt::fake())),
        &Machines::new().everywhere(Box::new(Nowhere)),
        &observed,
        &mut state,
        secrets(),
        &mut |_: &State| Ok(()),
    )
    .expect("applies");
    assert_eq!(
        state.get(&address(FAR_SERVICE)).unwrap().owed(),
        &BTreeSet::from([address(CONF)]),
        "owed on the record before anything else happens to it"
    );

    // The machine answers, and the file already matches.
    let mut back = Observed::new();
    back.record(
        address(CONF),
        [(
            "content".to_owned(),
            Value::Text("as it will be\n".to_owned()),
        )]
        .into_iter()
        .collect(),
    );
    back.record(
        address(FAR_SERVICE),
        [("name".to_owned(), Value::Text("nginx".to_owned()))]
            .into_iter()
            .collect(),
    );
    let next = plan(&graph, &back, &state, &Schema::new(), secrets()).expect("plannable");
    let service = next
        .changes()
        .find(|change| change.address() == &address(FAR_SERVICE))
        .expect("the service is in the plan");
    assert_eq!(service.kind(), ChangeKind::React, "{next:?}");
    assert_eq!(service.owed(), [address(CONF)]);
}

/// And a service with no record, which has nowhere to owe it, holds the file back
/// until its machine answers, and the plan says what waits.
#[test]
#[dacc_derive::doc_anchor(
    id = "a-cause-waits-while-what-it-rouses-cannot-be-planned-and-has-no-record"
)]
fn a_cause_waits_while_what_it_rouses_cannot_be_planned_and_has_no_record() {
    let (graph, observed, state) = far_world();

    let held = plan(&graph, &observed, &state, &Schema::new(), secrets()).expect("plannable");

    assert!(held.is_empty(), "the file waits: {held:?}");
    let out_of_reach: Vec<String> = held
        .unreachable()
        .iter()
        .map(|entry| entry.address().to_string())
        .collect();
    assert_eq!(
        out_of_reach,
        [FAR_SERVICE],
        "only the service is out of reach"
    );
    let waiting: Vec<(String, String)> = held
        .waiting()
        .iter()
        .map(|entry| (entry.address().to_string(), entry.on().to_string()))
        .collect();
    assert_eq!(
        waiting,
        [(CONF.to_owned(), FAR_SERVICE.to_owned())],
        "and the file, on a machine that answered, says it waits for it"
    );
}

/// And a cause on no machine, held for a reacting resource with no record on one
/// that did not answer, is named as waiting. It was named nowhere, because only
/// what is on a machine could be reported. Found by the review of RFC 0001's
/// fourth revision.
#[test]
#[dacc_derive::doc_anchor(id = "a-held-cause-on-no-machine-is-named-all-the-same")]
fn a_held_cause_on_no_machine_is_named_all_the_same() {
    let mut graph = Graph::new();
    graph.preexisting(nerpa_core::Target::new("web-02").unwrap());
    graph
        .insert(Resource::new(address("yandex:instance.db")).with("size", "large"))
        .unwrap();
    graph
        .insert(
            Resource::new(address(FAR_SERVICE))
                .with("name", "nginx")
                .reacting_to(address("yandex:instance.db")),
        )
        .unwrap();
    let mut observed = Observed::new();
    observed.record(
        address("yandex:instance.db"),
        [("size".to_owned(), Value::Text("small".to_owned()))]
            .into_iter()
            .collect(),
    );
    observed.unreachable(nerpa_core::Target::new("web-02").unwrap());
    let mut state = State::new();
    state.record(
        address("yandex:instance.db"),
        Record::new(BTreeMap::new(), BTreeSet::default()),
    );

    let held = plan(&graph, &observed, &state, &Schema::new(), secrets()).expect("plannable");

    assert!(held.is_empty(), "{held:?}");
    let waiting: Vec<(String, String)> = held
        .waiting()
        .iter()
        .map(|entry| (entry.address().to_string(), entry.on().to_string()))
        .collect();
    assert_eq!(
        waiting,
        [("yandex:instance.db".to_owned(), FAR_SERVICE.to_owned())]
    );
}

/// And a cause that is replaced leaves the reaction owed, the same as a cause that
/// is updated. A replacement was held instead, as though the far service were
/// inside the file. Found by the review of RFC 0001's fifth revision.
#[test]
#[dacc_derive::doc_anchor(
    id = "a-replaced-cause-owes-its-reaction-on-a-machine-that-did-not-answer"
)]
fn a_replaced_cause_owes_its_reaction_on_a_machine_that_did_not_answer() {
    let os = || nerpa_core::Provider::new("os").unwrap();
    let schema = Schema::new()
        .immutable(os(), nerpa_core::Kind::new("file").unwrap(), "path")
        .identity(os(), nerpa_core::Kind::new("file").unwrap(), "path", "path");
    let mut graph = Graph::new();
    for node in ["web-01", "web-02"] {
        graph.preexisting(nerpa_core::Target::new(node).unwrap());
    }
    graph
        .insert(
            Resource::new(address(CONF))
                .with("path", "/b")
                .with("content", "new\n"),
        )
        .unwrap();
    graph
        .insert(
            Resource::new(address(FAR_SERVICE))
                .with("name", "nginx")
                .reacting_to(address(CONF)),
        )
        .unwrap();
    let mut observed = Observed::new();
    observed.record(
        address(CONF),
        [
            ("path".to_owned(), Value::Text("/a".to_owned())),
            ("content".to_owned(), Value::Text("old\n".to_owned())),
        ]
        .into_iter()
        .collect(),
    );
    observed.unreachable(nerpa_core::Target::new("web-02").unwrap());
    let mut state = State::new();
    state.record(
        address(CONF),
        Record::new(BTreeMap::new(), BTreeSet::default()),
    );
    state.record(
        address(FAR_SERVICE),
        Record::new(BTreeMap::new(), [address(CONF)].into_iter().collect()),
    );

    let planned = plan(&graph, &observed, &state, &schema, secrets()).expect("plannable");
    let file = planned
        .changes()
        .find(|change| change.address() == &address(CONF))
        .expect("the replacement is planned");
    assert_eq!(file.kind(), ChangeKind::Replace, "{planned:?}");
    assert_eq!(file.owed_by(), [address(FAR_SERVICE)], "owing the reaction");
}

/// A plan with nothing out of reach, and something waiting, still has no digest,
/// and says which it is. It said it could not look at none of its machines, because
/// the refusal counted only what was out of reach. Found by the narrow review of RFC
/// 0001's sixth revision.
#[test]
fn a_plan_that_only_waits_says_so_where_its_number_would_be() {
    let mut graph = Graph::new();
    graph.preexisting(nerpa_core::Target::new("web-02").unwrap());
    graph
        .insert(Resource::new(address("yandex:instance.db")).with("size", "large"))
        .unwrap();
    graph
        .insert(
            Resource::new(address(FAR_SERVICE))
                .with("name", "nginx")
                .reacting_to(address("yandex:instance.db")),
        )
        .unwrap();
    let mut observed = Observed::new();
    observed.record(
        address("yandex:instance.db"),
        [("size".to_owned(), Value::Text("small".to_owned()))]
            .into_iter()
            .collect(),
    );
    observed.unreachable(nerpa_core::Target::new("web-02").unwrap());
    let mut state = State::new();
    state.record(
        address("yandex:instance.db"),
        Record::new(BTreeMap::new(), BTreeSet::default()),
    );

    let held = plan(&graph, &observed, &state, &Schema::new(), secrets()).expect("plannable");

    let refused = held.digest().expect_err("a plan that waits has no number");
    assert_eq!(refused.waiting, 1, "the cause waits: {refused}");
    assert_eq!(
        refused.machines, 1,
        "and the service is out of reach: {refused}"
    );
}
