//! One journey to a machine, however many resources are on it.
//!
//! This is the difference between reading a fleet in seconds and reading it in
//! minutes, and it is a property of the interface rather than of any transport:
//! an interface that can only ask about one resource makes the fast version
//! unrepresentable however good the connection underneath is.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::cell::RefCell;
use std::collections::BTreeMap;

use nerpa_core::address::address;
use nerpa_core::plan::ChangeKind;
use nerpa_core::provider::{
    Attributes, Machines, Provider, ProviderError, Providers, Reach, Reading,
};
use nerpa_core::run::observe;
use nerpa_core::schema::Schema;
use nerpa_core::secret::Recorded;
use nerpa_core::state::{Record, State};
use nerpa_core::{Address, ExecutionDomain, Graph, Resource, Value};

/// A far end nobody asks. These providers decide their answers themselves; the
/// seam is there because the trait has one, not because this uses it.
#[derive(Debug)]
struct Deaf;

impl Reach for Deaf {
    fn ask(&self, _ask: &nerpa_proto::Ask) -> Result<nerpa_proto::Answer, ProviderError> {
        Err(ProviderError::NotOnAMachine)
    }
}

fn machines() -> Machines {
    Machines::new().everywhere(Box::new(Deaf))
}

thread_local! {
    /// Every batch the provider was handed, in order. A thread local rather than
    /// state on the provider, because the registry takes ownership of it and a
    /// test cannot borrow back through a `Box<dyn Provider>`.
    static ASKED: RefCell<Vec<Vec<Reading>>> = const { RefCell::new(Vec::new()) };
}

fn watch() -> Providers {
    ASKED.with_borrow_mut(Vec::clear);
    Providers::new().with(Box::new(Watching))
}

fn asked() -> Vec<Vec<Reading>> {
    ASKED.with_borrow(Clone::clone)
}

/// Answers nothing, and remembers exactly what it was asked for.
#[derive(Debug)]
struct Watching;

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
        _reach: &dyn Reach,
    ) -> Result<BTreeMap<Address, Attributes>, ProviderError> {
        ASKED.with_borrow_mut(|seen| seen.push(batch.to_vec()));
        Ok(BTreeMap::new())
    }

    fn apply(
        &self,
        _address: &Address,
        _kind: ChangeKind,
        _attributes: &Attributes,
        _reach: &dyn Reach,
    ) -> Result<Attributes, ProviderError> {
        unreachable!("this provider is only ever read from")
    }

    fn delete(
        &self,
        _address: &Address,
        _recorded: &Attributes,
        _reach: &dyn Reach,
    ) -> Result<(), ProviderError> {
        unreachable!("this provider is only ever read from")
    }

    fn fake() -> Self {
        Self
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
    let providers = watch();

    observe(&graph, &providers, &machines(), &State::new()).unwrap();

    let calls = asked();
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
    let providers = watch();

    observe(&graph, &providers, &machines(), &State::new()).unwrap();

    let sizes: Vec<usize> = asked().iter().map(Vec::len).collect();
    assert_eq!(
        sizes,
        [1, 2],
        "one journey per machine: web-01 has one file, web-02 has two"
    );
}

#[test]
fn the_batch_carries_where_to_look() {
    let graph = graph_of(vec![file("motd", "web-01")]);
    let providers = watch();

    observe(&graph, &providers, &machines(), &State::new()).unwrap();

    let calls = asked();
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

    let providers = watch();
    observe(&graph, &providers, &machines(), &state).unwrap();

    assert_eq!(
        asked()[0][0].locator().get("path"),
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

    let providers = watch();
    observe(&graph, &providers, &machines(), &state).unwrap();

    let calls = asked();
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

    let refusal = observe(&graph, &watch(), &machines(), &State::new())
        .expect_err("a cycle is not a configuration that declares nothing");
    assert!(
        refusal.to_string().contains("os:file.a@web-01"),
        "the refusal should name what is stuck: {refusal}"
    );
    assert!(
        ASKED.with_borrow(Vec::is_empty),
        "and nothing should have been read at all"
    );
}
