//! The compatibility surface, checked in: a recorded configuration's plan is a
//! golden file here, and a change to the engine that changes one fails this
//! test until the golden is updated and a changelog entry is written
//! (INV-COMPAT-002).

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::{BTreeMap, BTreeSet};

use nerpa_core::address::address;
use nerpa_core::planner::plan;
use nerpa_core::secret::{Key, Recorded, Remembered, Secrets};
use nerpa_core::state::{Record, State};
use nerpa_core::value::Value;
use nerpa_core::{Graph, Observed, Resource, Schema};

/// A fixed key, so the plan digest in a golden file is reproducible.
static DIGEST_KEY: Key = Key::from_bytes([7; 32]);
static NO_STORE: Remembered = Remembered::EMPTY;

fn secrets() -> Secrets<'static> {
    Secrets::new(&DIGEST_KEY, &NO_STORE)
}

fn graph_of(resources: Vec<Resource>) -> Graph {
    let mut graph = Graph::new();
    for resource in resources {
        graph.insert(resource).expect("distinct addresses");
    }
    graph
}

fn observed_with(address_text: &str, values: &[(&str, i64)]) -> Observed {
    let mut observed = Observed::new();
    let recorded = values
        .iter()
        .map(|(name, value)| ((*name).to_owned(), Value::Integer(*value)))
        .collect::<BTreeMap<_, _>>();
    observed.record(address(address_text), recorded);
    observed
}

/// A state recording these plain values at this address.
fn record_at(address_text: &str, values: &[(&str, i64)]) -> State {
    let mut state = State::new();
    let attributes = Recorded::all(
        values
            .iter()
            .map(|(name, value)| ((*name).to_owned(), Value::Integer(*value))),
    );
    state.record(
        address(address_text),
        Record::new(attributes, BTreeSet::new()),
    );
    state
}

/// The plan for one recorded configuration, compared to its golden file.
#[dacc_derive::doc_anchor(id = "compat-check")]
fn check(golden: &str, graph: &Graph, observed: &Observed, state: &State) {
    let planned = plan(graph, observed, state, &Schema::new(), secrets()).expect("plannable");
    let actual = serde_json::to_string_pretty(&planned).expect("serializes");
    assert_eq!(
        actual.trim(),
        golden.trim(),
        "the plan changed; update the golden and write a changelog entry (INV-COMPAT-002)"
    );
}

#[test]
#[dacc_derive::doc_anchor(id = "compat-create")]
fn a_resource_that_does_not_exist_is_created() {
    let graph = graph_of(vec![
        Resource::new(address("yandex:instance.db")).with("cores", 8),
    ]);
    check(
        include_str!("corpus/create.json"),
        &graph,
        &Observed::new(),
        &State::new(),
    );
}

#[test]
#[dacc_derive::doc_anchor(id = "compat-update")]
fn a_resource_that_drifted_is_updated() {
    let graph = graph_of(vec![
        Resource::new(address("yandex:instance.db")).with("cores", 16),
    ]);
    check(
        include_str!("corpus/update.json"),
        &graph,
        &observed_with("yandex:instance.db", &[("cores", 8)]),
        &record_at("yandex:instance.db", &[("cores", 8)]),
    );
}

#[test]
#[dacc_derive::doc_anchor(id = "compat-delete")]
fn a_resource_no_longer_declared_is_deleted() {
    let graph = graph_of(vec![]);
    check(
        include_str!("corpus/delete.json"),
        &graph,
        &observed_with("yandex:instance.db", &[("cores", 8)]),
        &record_at("yandex:instance.db", &[("cores", 8)]),
    );
}

#[test]
#[dacc_derive::doc_anchor(id = "compat-rename")]
fn a_resource_renamed_keeps_its_identity() {
    let graph = graph_of(vec![
        Resource::new(address("yandex:instance.db")).with("cores", 8),
    ])
    .moved(
        address("yandex:instance.old"),
        address("yandex:instance.db"),
    );
    check(
        include_str!("corpus/rename.json"),
        &graph,
        &observed_with("yandex:instance.db", &[("cores", 8)]),
        &record_at("yandex:instance.old", &[("cores", 8)]),
    );
}
