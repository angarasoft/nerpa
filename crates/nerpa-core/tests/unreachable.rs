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
use nerpa_core::provider::{Attributes, Provider, ProviderError, Providers};
use nerpa_core::run::observe;
use nerpa_core::schema::Schema;
use nerpa_core::state::{Record, State, Written};
use nerpa_core::{Address, ExecutionDomain, Graph, Observed, Reference, Resource, Target};

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
            Attributes::new(),
            BTreeSet::new(),
            Written::at(1_700_000_000 - 3600),
        ),
    );

    let mut observed = Observed::new();
    observed.unreachable(node("web-01"));

    let plan = plan_at(&graph, &observed, &state, &Schema::new(), nothing_known()).unwrap();
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
        address: &Address,
        _declared: &Attributes,
    ) -> Result<Option<Attributes>, ProviderError> {
        match self.how {
            Refusal::Unreachable => Err(ProviderError::Unreachable {
                node: address.target().cloned().unwrap_or_else(|| node("nowhere")),
                problem: "no route to host".to_owned(),
            }),
            Refusal::Broken => Err(ProviderError::World {
                address: address.clone(),
                problem: "permission denied".to_owned(),
            }),
        }
    }

    fn apply(
        &self,
        _address: &Address,
        _kind: ChangeKind,
        _attributes: &Attributes,
    ) -> Result<Attributes, ProviderError> {
        unreachable!("this provider is only ever read from")
    }

    fn delete(&self, _address: &Address, _recorded: &Attributes) -> Result<(), ProviderError> {
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

    let observed = observe(&graph, &refusing(Refusal::Unreachable), &State::new())
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

    let refusal = observe(&graph, &refusing(Refusal::Broken), &State::new())
        .expect_err("permission denied is not a machine being down");

    assert!(
        refusal.to_string().contains("permission denied"),
        "{refusal}"
    );
}
