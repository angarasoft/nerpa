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
use nerpa_core::{
    Address, ExecutionDomain, Graph, Kind, Observed, Reference, Resource, Target, Value,
};

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

/// What a plan says is out of reach.
fn out_of_reach(plan: &nerpa_core::Plan) -> BTreeSet<String> {
    plan.unreachable()
        .iter()
        .map(|entry| entry.address().to_string())
        .collect()
}

/// What a plan says waits, and for what.
fn waiting(plan: &nerpa_core::Plan) -> Vec<(String, String)> {
    plan.waiting()
        .iter()
        .map(|entry| (entry.address().to_string(), entry.on().to_string()))
        .collect()
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
/// A removal is planned from state, and a machine that did not answer cannot be
/// asked to remove anything. It was planned anyway, in a plan that called itself
/// complete, and apply stopped at that first stage, before touching the machines
/// that did answer. Found by the third review of RFC 0001.
#[test]
fn a_removal_on_a_machine_that_did_not_answer_is_not_planned() {
    let mut state = State::new();
    state.record(
        address("os:file.gone@web-01"),
        Record::new(Recorded::all(Attributes::new()), BTreeSet::new()),
    );
    let mut observed = Observed::new();
    observed.unreachable(node("web-01"));

    let plan = plan_at(
        &Graph::new(),
        &observed,
        &state,
        &Schema::new(),
        secrets(),
        nothing_known(),
    )
    .unwrap();

    assert!(
        plan.is_empty(),
        "nothing is removed from a machine that did not answer: {plan:?}"
    );
    assert!(
        !plan.is_complete(),
        "and the plan does not call itself complete"
    );
    let named: BTreeSet<String> = plan
        .unreachable()
        .iter()
        .map(|entry| entry.address().to_string())
        .collect();
    assert!(named.contains("os:file.gone@web-01"), "{named:?}");
}

/// And a removal that has to wait for one it cannot have waits with it, while a
/// removal that waits for nothing still goes ahead. What a resource comes after is
/// removed after it, so removing that first would take it away from under a
/// resource still there.
#[test]
fn a_removal_waiting_on_one_that_cannot_be_made_is_held() {
    let mut state = State::new();
    state.record(
        address("os:file.source@web-02"),
        Record::new(Recorded::all(Attributes::new()), BTreeSet::new()),
    );
    state.record(
        address("os:file.reader@web-01"),
        Record::new(
            Recorded::all(Attributes::new()),
            [address("os:file.source@web-02")].into_iter().collect(),
        ),
    );
    state.record(
        address("os:file.alone@web-02"),
        Record::new(Recorded::all(Attributes::new()), BTreeSet::new()),
    );
    let mut observed = Observed::new();
    observed.unreachable(node("web-01"));

    let plan = plan_at(
        &Graph::new(),
        &observed,
        &state,
        &Schema::new(),
        secrets(),
        nothing_known(),
    )
    .unwrap();

    let planned: Vec<(String, ChangeKind)> = plan
        .changes()
        .map(|change| (change.address().to_string(), change.kind()))
        .collect();
    assert_eq!(
        planned,
        [("os:file.alone@web-02".to_owned(), ChangeKind::Delete)],
        "only what waits for nothing: {plan:?}"
    );
    assert_eq!(
        out_of_reach(&plan),
        BTreeSet::from(["os:file.reader@web-01".to_owned()])
    );
    assert_eq!(
        waiting(&plan),
        [(
            "os:file.source@web-02".to_owned(),
            "os:file.reader@web-01".to_owned()
        )],
        "and the removal waiting for it says so"
    );
}

/// And a declared resource naming what a held removal takes away waits with it. It
/// used to be adopted from the reading, and the removal, planned in whichever later
/// run its machine answered, took the file away from under that record. Found while
/// ordering removals after the narrow review of RFC 0001.
#[test]
fn a_resource_naming_what_a_held_removal_takes_is_held_with_it() {
    let schema = Schema::new().identity(
        nerpa_core::Provider::new("os").unwrap(),
        Kind::new("file").unwrap(),
        "path",
        "path",
    );
    let mut state = State::new();
    state.record(
        address("os:file.source@web-02"),
        Record::new(
            Recorded::all(
                [("path".to_owned(), Value::Text("/srv/a".to_owned()))]
                    .into_iter()
                    .collect::<Attributes>(),
            ),
            BTreeSet::new(),
        ),
    );
    state.record(
        address("os:file.reader@web-01"),
        Record::new(
            Recorded::all(Attributes::new()),
            [address("os:file.source@web-02")].into_iter().collect(),
        ),
    );
    let graph = graph_of(vec![
        Resource::new(address("os:file.successor@web-02"))
            .with("path", "/srv/a")
            .with("content", "hello\n"),
    ]);
    let mut observed = Observed::new();
    observed.unreachable(node("web-01"));
    observed.record(
        address("os:file.successor@web-02"),
        [
            ("path".to_owned(), Value::Text("/srv/a".to_owned())),
            ("content".to_owned(), Value::Text("hello\n".to_owned())),
        ]
        .into_iter()
        .collect(),
    );

    let plan = plan_at(
        &graph,
        &observed,
        &state,
        &schema,
        secrets(),
        nothing_known(),
    )
    .unwrap();

    assert!(
        plan.is_empty(),
        "nothing on either side of a removal that cannot happen yet: {plan:?}"
    );
    assert_eq!(
        waiting(&plan),
        [
            (
                "os:file.source@web-02".to_owned(),
                "os:file.reader@web-01".to_owned()
            ),
            (
                "os:file.successor@web-02".to_owned(),
                "os:file.source@web-02".to_owned()
            ),
        ],
        "and the plan says what each waits for"
    );
}

/// And a removal waits while what state recorded after it cannot be planned:
/// whether that is still in the way is not known until its machine answers.
#[test]
fn a_removal_waits_while_what_was_recorded_after_it_cannot_be_planned() {
    let mut state = State::new();
    state.record(
        address("os:directory.old@web-02"),
        Record::new(Recorded::all(Attributes::new()), BTreeSet::new()),
    );
    state.record(
        address("os:file.conf@web-01"),
        Record::new(
            Recorded::all(Attributes::new()),
            [address("os:directory.old@web-02")].into_iter().collect(),
        ),
    );
    let graph = graph_of(vec![
        Resource::new(address("os:file.conf@web-01")).with("content", "moved\n"),
    ]);
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

    assert!(plan.is_empty(), "{plan:?}");
    assert_eq!(
        out_of_reach(&plan),
        BTreeSet::from(["os:file.conf@web-01".to_owned()])
    );
    assert_eq!(
        waiting(&plan),
        [(
            "os:directory.old@web-02".to_owned(),
            "os:file.conf@web-01".to_owned()
        )],
        "both said, each as what it is"
    );
}

/// And a replacement waits while what state recorded after it cannot be planned:
/// removing what was there could take it away from under something still in it.
#[test]
fn a_replacement_waits_while_what_was_recorded_inside_it_cannot_be_planned() {
    let schema = Schema::new().immutable(
        nerpa_core::Provider::new("os").unwrap(),
        Kind::new("directory").unwrap(),
        "path",
    );
    let mut state = State::new();
    state.record(
        address("os:directory.d@web-01"),
        Record::new(
            Recorded::all(
                [("path".to_owned(), Value::Text("/d".to_owned()))]
                    .into_iter()
                    .collect::<Attributes>(),
            ),
            BTreeSet::new(),
        ),
    );
    state.record(
        address("os:file.inside@web-02"),
        Record::new(
            Recorded::all(Attributes::new()),
            [address("os:directory.d@web-01")].into_iter().collect(),
        ),
    );
    let graph = graph_of(vec![
        Resource::new(address("os:directory.d@web-01")).with("path", "/e"),
        Resource::new(address("os:file.inside@web-02")).with("content", "x\n"),
    ]);
    let mut observed = Observed::new();
    observed.unreachable(node("web-02"));
    observed.record(
        address("os:directory.d@web-01"),
        [("path".to_owned(), Value::Text("/d".to_owned()))]
            .into_iter()
            .collect(),
    );

    let plan = plan_at(
        &graph,
        &observed,
        &state,
        &schema,
        secrets(),
        nothing_known(),
    )
    .unwrap();

    assert!(plan.is_empty(), "{plan:?}");
    assert_eq!(
        out_of_reach(&plan),
        BTreeSet::from(["os:file.inside@web-02".to_owned()])
    );
    assert_eq!(
        waiting(&plan),
        [(
            "os:directory.d@web-01".to_owned(),
            "os:file.inside@web-02".to_owned()
        )]
    );
}

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
        out_of_reach(&plan),
        BTreeSet::from(["os:directory.data@web-01".to_owned()]),
        "only what is on the machine that did not answer"
    );
    assert_eq!(
        waiting(&plan),
        [(
            "os:file.marker@web-02".to_owned(),
            "os:directory.data@web-01".to_owned()
        )],
        "the reader has nowhere to read from, and says what it waits for"
    );
    assert_eq!(plan.blast_radius().waiting(), 1);
    assert!(plan.is_empty() && !plan.is_complete());
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
