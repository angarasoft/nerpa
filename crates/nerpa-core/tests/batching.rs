//! One journey to a machine, however many resources are on it.
//!
//! This is the difference between reading a fleet in seconds and reading it in
//! minutes, and it is a property of the interface rather than of any transport:
//! an interface that can only ask about one resource makes the fast version
//! unrepresentable however good the connection underneath is.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::sync::{Arc, Mutex};

use nerpa_core::address::address;
use nerpa_core::plan::ChangeKind;
use nerpa_core::provider::{
    Attributes, Machines, Provider, ProviderError, Providers, Reach, Read, Reading,
};
use nerpa_core::run::observe;
use nerpa_core::schema::Schema;
use nerpa_core::secret::Recorded;
use nerpa_core::state::{Record, State};
use nerpa_core::{Address, ExecutionDomain, Graph, Resource, Value};

/// A machine that answers and knows nothing about itself.
///
/// The providers in these tests decide their own answers, so the seam is here
/// because the trait has one. It answers rather than refuses because a far end
/// that refuses is a machine out of reach, and these machines are present — they
/// just have nothing to say about what they are.
#[derive(Debug)]
struct Blank;

impl Reach for Blank {
    fn ask(&self, ask: &nerpa_proto::Ask) -> Result<nerpa_proto::Answer, ProviderError> {
        match ask {
            nerpa_proto::Ask::Look { observations, .. } => Ok(nerpa_proto::Answer::Found {
                seen: observations
                    .iter()
                    .map(|_| nerpa_proto::Seen::Nothing)
                    .collect(),
            }),
            _ => Err(ProviderError::NotOnAMachine),
        }
    }
}

fn machines() -> Machines {
    Machines::new().everywhere(Box::new(Blank))
}

/// Every batch the provider was handed.
///
/// Held by the provider and by the test at once, which is what `Arc` is for —
/// the registry takes ownership of the provider and a test cannot borrow back
/// through a `Box<dyn Provider>`. It used to be a thread local, and that stopped
/// working the day separate machines began to be spoken to on separate threads:
/// a recorder each thread kept to itself had the test reading an empty list and
/// calling it a pass, which is worse than failing. A single shared static would
/// have been worse again, because the test harness runs these in parallel and
/// they would have cleared each other's.
#[derive(Debug, Clone, Default)]
struct Ledger(Arc<Mutex<Vec<Vec<Reading>>>>);

impl Ledger {
    fn note(&self, batch: &[Reading]) {
        if let Ok(mut seen) = self.0.lock() {
            seen.push(batch.to_vec());
        }
    }

    /// In the order the batches were built rather than the order threads
    /// finished, so what this asserts does not depend on which machine was
    /// quickest.
    fn asked(&self) -> Vec<Vec<Reading>> {
        let mut seen = self.0.lock().map(|seen| seen.clone()).unwrap_or_default();
        // By machine, which is what a batch belongs to. Sorting by the first
        // address in it would order by whichever file happened to come first
        // and put the second machine ahead of the first.
        seen.sort_by_key(|batch| {
            batch
                .first()
                .and_then(|reading| reading.address().target().map(ToString::to_string))
                .unwrap_or_default()
        });
        seen
    }
}

fn watch() -> (Providers, Ledger) {
    let ledger = Ledger::default();
    (
        Providers::new().with(Box::new(Watching(ledger.clone()))),
        ledger,
    )
}

/// Answers nothing, and remembers exactly what it was asked for.
#[derive(Debug)]
struct Watching(Ledger);

impl Provider for Watching {
    fn name(&self) -> &nerpa_core::Provider {
        static NAME: std::sync::OnceLock<nerpa_core::Provider> = std::sync::OnceLock::new();
        NAME.get_or_init(|| nerpa_core::Provider::new("os").unwrap())
    }

    fn schema(&self) -> Schema {
        Schema::new()
    }

    fn read(
        &self,
        batch: &[Reading],
        _facts: nerpa_core::observed::Facts<'_>,
        _reach: &dyn Reach,
    ) -> Result<Read, ProviderError> {
        self.0.note(batch);
        Ok(Read::new())
    }

    fn apply(
        &self,
        _address: &Address,
        _kind: ChangeKind,
        _roused: bool,
        _attributes: &Attributes,
        _premise: &[nerpa_proto::Premise],
        _facts: nerpa_core::observed::Facts<'_>,
        _reach: &dyn Reach,
    ) -> Result<Attributes, ProviderError> {
        unreachable!("this provider is only ever read from")
    }

    fn delete(
        &self,
        _address: &Address,
        _recorded: &Attributes,
        _premise: &[nerpa_proto::Premise],
        _facts: nerpa_core::observed::Facts<'_>,
        _reach: &dyn Reach,
    ) -> Result<(), ProviderError> {
        unreachable!("this provider is only ever read from")
    }

    fn fake() -> Self {
        Self(Ledger::default())
    }
}

fn graph_of(resources: Vec<Resource>) -> Graph {
    let mut graph = Graph::new();
    for resource in resources {
        if let ExecutionDomain::Target(target) = resource.address().domain() {
            graph.preexisting(target);
        }
        graph.insert(resource).unwrap();
    }
    graph
}

fn file(name: &str, node: &str) -> Resource {
    Resource::new(address(&format!("os:file.{name}@{node}"))).with("path", format!("/etc/{name}"))
}

#[test]
fn every_resource_on_one_machine_is_asked_for_at_once() {
    let graph = graph_of(vec![
        file("motd", "web-01"),
        file("hosts", "web-01"),
        file("resolv", "web-01"),
    ]);
    let (providers, ledger) = watch();

    observe(&graph, &providers, &machines(), &State::new()).unwrap();

    let calls = ledger.asked();
    assert_eq!(
        calls.len(),
        1,
        "three files on one machine is one journey, not three"
    );
    assert_eq!(calls[0].len(), 3);
}

#[test]
fn separate_machines_are_separate_journeys() {
    let graph = graph_of(vec![
        file("motd", "web-01"),
        file("motd", "web-02"),
        file("hosts", "web-02"),
    ]);
    let (providers, ledger) = watch();

    observe(&graph, &providers, &machines(), &State::new()).unwrap();

    let sizes: Vec<usize> = ledger.asked().iter().map(Vec::len).collect();
    assert_eq!(
        sizes,
        [1, 2],
        "one journey per machine: web-01 has one file, web-02 has two"
    );
}

#[test]
fn the_batch_carries_where_to_look() {
    let graph = graph_of(vec![file("motd", "web-01")]);
    let (providers, ledger) = watch();

    observe(&graph, &providers, &machines(), &State::new()).unwrap();

    let calls = ledger.asked();
    assert_eq!(
        calls[0][0].locator().get("path"),
        Some(&Value::Text("/etc/motd".to_owned())),
        "the declaration's path travelled with the request"
    );
    assert_eq!(calls[0][0].address(), &address("os:file.motd@web-01"));
}

/// For a resource state remembers, where to look is where it was left — not
/// where the declaration now points.
// ENFORCES: INV-STATE-003
#[test]
fn what_state_remembers_beats_what_the_declaration_says() {
    let graph = graph_of(vec![file("motd", "web-01")]);

    let mut state = State::new();
    let mut recorded = Attributes::new();
    recorded.insert("path".to_owned(), Value::Text("/var/motd".to_owned()));
    state.record(
        address("os:file.motd@web-01"),
        Record::new(Recorded::all(recorded), std::collections::BTreeSet::new()),
    );

    let (providers, ledger) = watch();
    observe(&graph, &providers, &machines(), &state).unwrap();

    assert_eq!(
        ledger.asked()[0][0].locator().get("path"),
        Some(&Value::Text("/var/motd".to_owned())),
        "looked where it was left, not where it should be"
    );
}

/// A resource the configuration no longer declares still has to be read, or its
/// removal would be planned blind — and it joins its machine's journey.
#[test]
fn something_only_state_remembers_travels_with_the_rest() {
    let graph = graph_of(vec![file("motd", "web-01")]);

    let mut state = State::new();
    state.record(
        address("os:file.forgotten@web-01"),
        Record::new(
            Recorded::all(Attributes::new()),
            std::collections::BTreeSet::new(),
        ),
    );

    let (providers, ledger) = watch();
    observe(&graph, &providers, &machines(), &state).unwrap();

    let calls = ledger.asked();
    assert_eq!(calls.len(), 1, "still one journey to that machine");
    let names: Vec<String> = calls[0].iter().map(|r| r.address().to_string()).collect();
    assert_eq!(names, ["os:file.forgotten@web-01", "os:file.motd@web-01"]);
}

/// A configuration that cannot be ordered is a refusal, not an empty one.
///
/// `stages()` returns a `Result`, and `.into_iter().flatten()` on one yields
/// nothing for an error — so a cycle used to read as a configuration declaring
/// no resources at all. Everything downstream then looked only at what state
/// remembered, which is the shape of silent wrongness this project exists to
/// avoid: the run continues, and it continues about the wrong thing.
#[test]
fn a_graph_that_cannot_be_ordered_stops_the_run_rather_than_reading_as_empty() {
    let mut graph = Graph::new();
    for (name, after) in [("a", "os:file.b@web-01"), ("b", "os:file.a@web-01")] {
        let address: Address = format!("os:file.{name}@web-01")
            .parse()
            .expect("an address");
        graph.preexisting(address.target().expect("on a node").clone());
        graph
            .insert(
                Resource::new(address)
                    .with("path", format!("/tmp/{name}"))
                    .after(after.parse::<Address>().expect("an address")),
            )
            .expect("distinct addresses");
    }

    let (providers, ledger) = watch();
    let refusal = observe(&graph, &providers, &machines(), &State::new())
        .expect_err("a cycle is not a configuration that declares nothing");
    assert!(
        refusal.to_string().contains("os:file.a@web-01"),
        "the refusal should name what is stuck: {refusal}"
    );
    assert!(
        ledger.asked().is_empty(),
        "and nothing should have been read at all"
    );
}
