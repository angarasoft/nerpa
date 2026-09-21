//! What a change left in flight does to the next plan.
//!
//! A run that ended while a change was in flight left a machine nobody can
//! describe: the request may have arrived, or not, and the answer is gone with
//! the run. So that machine is held — nothing is read from it and nothing is
//! planned on it — until a person says otherwise. RFC 0001 §5.8.

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
use nerpa_core::secret::{Key, Remembered, Secrets};
use nerpa_core::state::{Mark, Released, State, Written};
use nerpa_core::{Graph, Plan, Resource, Target, Value};

/// A machine that answers and knows nothing about itself.
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

/// Answers about nothing, so every declared resource has to be created — and
/// planning has something registered to ask.
#[derive(Debug)]
struct Empty;

impl Provider for Empty {
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
        _change: &nerpa_core::provider::Prepared<'_>,
    ) -> Result<nerpa_core::provider::Planned, ProviderError> {
        unreachable!("nothing is applied in these tests")
    }

    fn conclude(
        &self,
        _change: &nerpa_core::provider::Concluding<'_>,
        _became: &[nerpa_core::provider::Answered],
    ) -> Result<nerpa_core::provider::Concluded, ProviderError> {
        unreachable!("nothing is applied in these tests")
    }

    fn afterwards(
        &self,
        _address: &nerpa_core::Address,
        _kind: nerpa_core::provider::Sending,
    ) -> nerpa_core::provider::Afterwards {
        nerpa_core::provider::Afterwards::Read
    }

    fn fake() -> Self {
        Self
    }
}

static DIGEST_KEY: Key = Key::from_bytes([7; 32]);
static NO_STORE: Remembered = Remembered::EMPTY;

fn secrets() -> Secrets<'static> {
    Secrets::new(&DIGEST_KEY, &NO_STORE)
}

/// Two files, one on each machine, and the second coming after the first where
/// `after` says so.
fn graph_of(after: bool) -> Graph {
    let mut graph = Graph::new();
    graph.preexisting(Target::new("web-01").unwrap());
    graph.preexisting(Target::new("web-02").unwrap());
    graph
        .insert(
            Resource::new(address("os:file.motd@web-01"))
                .with("path", Value::Text("/etc/motd".to_owned())),
        )
        .unwrap();
    let second = Resource::new(address("os:file.issue@web-01"))
        .with("path", Value::Text("/etc/issue".to_owned()));
    let second = if after {
        second.after(address("os:file.motd@web-01"))
    } else {
        second
    };
    graph.insert(second).unwrap();
    graph
        .insert(
            Resource::new(address("os:file.motd@web-02"))
                .with("path", Value::Text("/etc/motd".to_owned())),
        )
        .unwrap();
    graph
}

/// State with one change left in flight on web-01.
fn in_flight() -> State {
    let mut state = State::new();
    let mut locator = Attributes::new();
    locator.insert("path".to_owned(), Value::Text("/etc/motd".to_owned()));
    state.mark(
        address("os:file.motd@web-01"),
        Mark::new(ChangeKind::Update, locator, Attributes::new())
            .written_at(Written::at(1_700_000_000)),
    );
    state
}

fn planned(graph: &Graph, state: &State) -> Plan {
    let providers = Providers::new().with(Box::new(Empty));
    let machines = Machines::new().everywhere(Box::new(Blank));
    let observed = observe(graph, &providers, &machines, state).expect("readable");
    plan_at(
        graph,
        &observed,
        state,
        &Schema::new(),
        secrets(),
        Written::at(1_700_000_100),
    )
    .expect("plannable")
}

#[test]
fn a_machine_with_a_change_in_flight_is_held() {
    let graph = graph_of(false);
    let plan = planned(&graph, &in_flight());

    let held: BTreeSet<String> = plan
        .held()
        .iter()
        .map(|held| format!("{:?}", held.domain()))
        .collect();
    assert_eq!(held.len(), 1, "one machine is held: {held:?}");
    assert!(
        !plan.is_complete(),
        "a plan that could not look at a machine is not complete"
    );

    let planned: BTreeSet<String> = plan
        .changes()
        .map(|change| change.address().to_string())
        .collect();
    assert!(
        planned.contains("os:file.motd@web-02"),
        "the machine that is not held is planned as usual: {planned:?}"
    );
    assert!(
        !planned.iter().any(|address| address.ends_with("@web-01")),
        "and nothing on the held one is: {planned:?}"
    );
}

#[test]
fn a_held_machine_is_not_reported_as_one_that_did_not_answer() {
    let graph = graph_of(false);
    let plan = planned(&graph, &in_flight());

    assert!(
        plan.unreachable().is_empty(),
        "it answered; it is held, which is a different thing to tell somebody: {:?}",
        plan.unreachable()
    );
}

/// Releasing a machine settles the changes that were in flight on it.
///
/// A hold has to have a way out, or a run that was killed takes the machine with
/// it for good. Only a person can give it: nothing here can tell whether the
/// change arrived, so somebody who can look at the machine says it is theirs
/// again. What was in flight does not vanish — it becomes what the next plan has
/// to deal with. RFC 0001 §5.8.
#[test]
fn releasing_a_machine_settles_what_was_in_flight_on_it() {
    let mut state = in_flight();
    let released = state.release(Some(&Target::new("web-01").unwrap()));

    assert_eq!(released, 1, "one change was in flight there");
    let mark = state
        .marked(&address("os:file.motd@web-01"))
        .expect("what it was doing is not thrown away");
    assert!(
        !mark.in_flight(),
        "nothing is in flight there any more: {:?}",
        mark.stands()
    );

    let graph = graph_of(false);
    let plan = planned(&graph, &state);
    assert!(
        plan.held().is_empty(),
        "and the machine is not held: {:?}",
        plan.held()
    );
}

/// And releasing one machine says nothing about another.
#[test]
fn releasing_one_machine_leaves_the_others_as_they_were() {
    let mut state = in_flight();
    let released = state.release(Some(&Target::new("web-02").unwrap()));

    assert_eq!(released, 0, "nothing was in flight there");
    assert!(
        state
            .marked(&address("os:file.motd@web-01"))
            .is_some_and(Mark::in_flight),
        "the machine that was left mid-change still is"
    );
}

/// And an entry whose declaration is still there is not removed.
///
/// A removal is worked out from what state remembers and what the configuration
/// still asks for, and an entry is remembered — so the set it is compared
/// against has to be the same set. Compared against records alone, an address
/// remembered only by an entry looked undeclared while its declaration was
/// right there, and the plan both removed it and made it again. RFC 0001 §5.6.
#[test]
fn an_entry_whose_declaration_is_still_there_is_not_removed() {
    let mut state = in_flight();
    state.release(Some(&Target::new("web-01").unwrap()));

    let graph = graph_of(false);
    let plan = planned(&graph, &state);

    let removals: Vec<String> = plan
        .changes()
        .filter(|change| change.kind() == ChangeKind::Delete)
        .map(|change| change.address().to_string())
        .collect();
    assert!(
        removals.is_empty(),
        "nothing declared is taken away: {removals:?}"
    );
}

/// An entry whose declaration is gone is removed, at the locator it remembers.
///
/// A change that failed after its operations ran may have made the thing. State
/// has no record of it — the record is written from what the change became, and
/// it never became anything — so the only thing that knows where to look is the
/// entry. Without this, what a failed change made stays on the machine with
/// nothing recording it, which is the state decision 0026 exists to make
/// unreachable. RFC 0001 §5.6.
#[test]
fn an_entry_whose_declaration_is_gone_is_removed() {
    let mut state = in_flight();
    state.release(Some(&Target::new("web-01").unwrap()));

    // Nothing is declared any more.
    let mut graph = Graph::new();
    graph.preexisting(Target::new("web-01").unwrap());
    let plan = planned(&graph, &state);

    let removals: Vec<String> = plan
        .changes()
        .filter(|change| change.kind() == ChangeKind::Delete)
        .map(|change| change.address().to_string())
        .collect();
    assert_eq!(
        removals,
        vec!["os:file.motd@web-01".to_owned()],
        "what the failed change may have made is taken away: {:?}",
        plan.changes()
            .map(|change| change.address().to_string())
            .collect::<Vec<_>>()
    );
}

/// A person can clear one entry, which is them saying they looked.
///
/// Nothing else can: reading cannot settle the change, or it would not be an
/// entry. So the last way out is somebody who can see the machine saying there
/// is nothing out there — and from then on Nerpa has nothing to remove and
/// says so by not planning it. RFC 0001 §5.6.
#[test]
fn a_person_can_clear_one_entry() {
    let mut state = in_flight();
    state.release(Some(&Target::new("web-01").unwrap()));

    let Released::Cleared(mark) = state.release_entry(&address("os:file.motd@web-01")) else {
        panic!("there is an entry there to clear");
    };
    assert_eq!(mark.kind(), ChangeKind::Update, "and it says what it was");
    assert_eq!(
        state.release_entry(&address("os:file.motd@web-01")),
        Released::Nothing,
        "nothing is remembered there now"
    );

    // Nothing is declared any more, and nothing is removed either.
    let mut graph = Graph::new();
    graph.preexisting(Target::new("web-01").unwrap());
    let plan = planned(&graph, &state);
    assert!(
        plan.changes().next().is_none(),
        "what a person took responsibility for is not Nerpa's to remove: {:?}",
        plan.changes()
            .map(|change| change.address().to_string())
            .collect::<Vec<_>>()
    );
}

/// A change still in flight is not an entry, and is not cleared as one.
///
/// It belongs to a run nobody has accounted for: its machine is held, and that
/// hold is the only thing saying a change is out there. Dropping the mark would
/// take the hold with it and leave nothing saying anything.
#[test]
fn a_change_still_in_flight_is_not_cleared_as_an_entry() {
    let mut state = in_flight();

    assert_eq!(
        state.release_entry(&address("os:file.motd@web-01")),
        Released::InFlight,
        "the machine is released first, which is a smaller thing to say"
    );
    assert!(
        state
            .marked(&address("os:file.motd@web-01"))
            .is_some_and(Mark::in_flight),
        "and it is left exactly as it was"
    );
}
