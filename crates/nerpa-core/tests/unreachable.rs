//! What a machine that will not answer does to a plan.
//!
//! The decision is 0020: the unit of refusal is the resource, not the run, and
//! nothing is claimed about what could not be looked at.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::BTreeSet;

use nerpa_core::address::address;
use nerpa_core::plan::ChangeKind;
use nerpa_core::planner::plan_at;
use nerpa_core::provider::{
    Attributes, Machines, Provider, ProviderError, Providers, Reach, Read, Reading,
};
use nerpa_core::run::observe;
use nerpa_core::schema::Schema;
use nerpa_core::secret::{Key, Recorded, Remembered, Secrets};
use nerpa_core::state::{Record, State, Written};
use nerpa_core::{Address, ExecutionDomain, Graph, Observed, Reference, Resource, Target};

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

/// A fixed key and a store holding nothing.
///
/// No test in this file declares a secret; planning still has to be told where
/// one would be fetched from, and a fixed key keeps digests reproducible.
static DIGEST_KEY: Key = Key::from_bytes([7; 32]);
static NO_STORE: Remembered = Remembered::EMPTY;

fn secrets() -> Secrets<'static> {
    Secrets::new(&DIGEST_KEY, &NO_STORE)
}

fn node(name: &str) -> Target {
    Target::new(name).unwrap()
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

fn nothing_known() -> Written {
    Written::at(1_700_000_000)
}

#[test]
fn a_resource_on_a_machine_that_did_not_answer_is_not_planned() {
    let graph = graph_of(vec![
        Resource::new(address("os:file.motd@web-01")).with("path", "/etc/motd"),
        Resource::new(address("yandex:instance.db")).with("cores", 8),
    ]);

    let mut observed = Observed::new();
    observed.unreachable(node("web-01"));

    let plan = plan_at(
        &graph,
        &observed,
        &State::new(),
        &Schema::new(),
        secrets(),
        nothing_known(),
    )
    .unwrap();

    // The cloud resource is still planned: one machine being down does not stop
    // everything else.
    assert_eq!(plan.blast_radius().creates(), 1);
    assert_eq!(
        plan.changes().next().unwrap().address(),
        &address("yandex:instance.db")
    );

    assert_eq!(plan.blast_radius().unreachable(), 1);
    assert!(
        !plan.is_complete(),
        "a plan with a machine it could not see is not complete"
    );
    assert_eq!(
        plan.unreachable()[0].address(),
        &address("os:file.motd@web-01")
    );
    assert_eq!(plan.unreachable()[0].node(), &node("web-01"));
}

/// Nothing is claimed about it, and that includes not claiming it is unchanged.
#[test]
fn nothing_at_all_is_said_about_what_could_not_be_read() {
    let graph = graph_of(vec![
        Resource::new(address("os:file.motd@web-01")).with("content", "hello\n"),
    ]);

    let mut observed = Observed::new();
    observed.unreachable(node("web-01"));

    let plan = plan_at(
        &graph,
        &observed,
        &State::new(),
        &Schema::new(),
        secrets(),
        nothing_known(),
    )
    .unwrap();

    assert!(
        plan.is_empty(),
        "there is no work, because there is no knowledge"
    );
    assert_eq!(plan.changes().count(), 0);
    assert_eq!(plan.unreachable().len(), 1);
    assert_eq!(
        plan.unreachable()[0].known_for(),
        None,
        "never seen, so no age to give"
    );
}

/// Unreachability travels the same way an unresolved value does.
#[test]
fn a_resource_that_reads_from_an_unreachable_one_cannot_be_planned_either() {
    let graph = graph_of(vec![
        Resource::new(address("os:directory.data@web-01")).with("path", "/data"),
        Resource::new(address("os:file.marker@web-02")).with(
            "content",
            Reference::to(address("os:directory.data@web-01"), "path"),
        ),
    ]);

    let mut observed = Observed::new();
    observed.unreachable(node("web-01"));

    let plan = plan_at(
        &graph,
        &observed,
        &State::new(),
        &Schema::new(),
        secrets(),
        nothing_known(),
    )
    .unwrap();

    assert_eq!(
        plan.blast_radius().unreachable(),
        2,
        "the reader has nowhere to read from"
    );
    let named: BTreeSet<String> = plan
        .unreachable()
        .iter()
        .map(|entry| entry.address().to_string())
        .collect();
    assert!(named.contains("os:file.marker@web-02"), "{named:?}");
    assert!(plan.is_empty());
}

#[test]
fn the_age_of_what_is_known_comes_from_state() {
    let graph = graph_of(vec![
        Resource::new(address("os:file.motd@web-01")).with("path", "/etc/motd"),
    ]);

    let mut state = State::new();
    state.record(
        address("os:file.motd@web-01"),
        Record::written_at(
            Recorded::all(Attributes::new()),
            BTreeSet::new(),
            Written::at(1_700_000_000 - 3600),
        ),
    );

    let mut observed = Observed::new();
    observed.unreachable(node("web-01"));

    let plan = plan_at(
        &graph,
        &observed,
        &state,
        &Schema::new(),
        secrets(),
        nothing_known(),
    )
    .unwrap();
    assert_eq!(
        plan.unreachable()[0].known_for(),
        Some(3600),
        "an hour since it was last seen"
    );
}

#[test]
fn a_machine_that_answered_is_planned_as_usual() {
    let graph = graph_of(vec![
        Resource::new(address("os:file.motd@web-01")).with("content", "hello\n"),
    ]);

    let plan = plan_at(
        &graph,
        &Observed::new(),
        &State::new(),
        &Schema::new(),
        secrets(),
        nothing_known(),
    )
    .unwrap();

    assert!(plan.is_complete());
    assert_eq!(plan.blast_radius().unreachable(), 0);
    assert_eq!(plan.changes().next().unwrap().kind(), ChangeKind::Create);
}

/// A provider that refuses in whichever way the test needs.
#[derive(Debug)]
struct Refusing {
    name: nerpa_core::Provider,
    how: Refusal,
}

#[derive(Debug, Clone, Copy)]
enum Refusal {
    Unreachable,
    Broken,
}

impl Provider for Refusing {
    fn name(&self) -> &nerpa_core::Provider {
        &self.name
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
        let address = batch
            .first()
            .map(Reading::address)
            .cloned()
            .unwrap_or_else(|| address("os:file.nothing@nowhere"));

        match self.how {
            Refusal::Unreachable => Err(ProviderError::Unreachable {
                node: address.target().cloned().unwrap_or_else(|| node("nowhere")),
                problem: "no route to host".to_owned(),
            }),
            Refusal::Broken => Err(ProviderError::World {
                address,
                problem: "permission denied".to_owned(),
            }),
        }
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
        Self {
            name: nerpa_core::Provider::new("os").unwrap(),
            how: Refusal::Unreachable,
        }
    }
}

fn refusing(how: Refusal) -> Providers {
    Providers::new().with(Box::new(Refusing {
        name: nerpa_core::Provider::new("os").unwrap(),
        how,
    }))
}

#[test]
fn a_machine_not_answering_lets_the_reading_continue() {
    let graph = graph_of(vec![
        Resource::new(address("os:file.motd@web-01")).with("path", "/etc/motd"),
    ]);

    let observed = observe(
        &graph,
        &refusing(Refusal::Unreachable),
        &machines(),
        &State::new(),
    )
    .expect("one machine being down is not a reason to stop");

    assert!(!observed.reached(&node("web-01")));
    assert_eq!(observed.unreachable_nodes().len(), 1);
}

/// The distinction the whole decision rests on. Treating this as unreachability
/// would quietly stop managing a resource because of a typo in an access rule.
#[test]
fn any_other_refusal_still_stops_the_run() {
    let graph = graph_of(vec![
        Resource::new(address("os:file.motd@web-01")).with("path", "/etc/motd"),
    ]);

    let refusal = observe(
        &graph,
        &refusing(Refusal::Broken),
        &machines(),
        &State::new(),
    )
    .expect_err("permission denied is not a machine being down");

    assert!(
        refusal.to_string().contains("permission denied"),
        "{refusal}"
    );
}
