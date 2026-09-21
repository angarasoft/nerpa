//! What an entry does to the next plan.
//!
//! An entry is a change an earlier run marked and never recorded: its
//! operations ran, and the answer went with the run. Nobody knows whether it
//! took. So it is not adopted and it is not passed over — it goes again, where
//! this run's reading leaves the declaration's own change with nothing to do.
//! RFC 0001 §5.6.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use nerpa_core::address::address;
use nerpa_core::plan::{Change, ChangeKind};
use nerpa_core::planner::plan_at;
use nerpa_core::provider::{
    Afterwards, Attributes, Machines, Provider, ProviderError, Providers, Reach, Read, Reading,
    Sending,
};
use nerpa_core::run::observe;
use nerpa_core::schema::Schema;
use nerpa_core::secret::{Key, Remembered, Secrets};
use nerpa_core::state::{Mark, Standing, State, Written};
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

/// Answers that every file asked about is there, where it was looked for, with
/// the content this was built with.
#[derive(Debug)]
struct Holding(&'static str);

impl Provider for Holding {
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
        let mut read = Read::new();
        for reading in batch {
            let mut found = reading.locator().clone();
            found.insert("content".to_owned(), Value::Text(self.0.to_owned()));
            read = read.with(reading.address().clone(), found, Vec::new());
        }
        Ok(read)
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

    fn afterwards(&self, _address: &nerpa_core::Address, _kind: Sending) -> Afterwards {
        Afterwards::SendAgain
    }

    fn fake() -> Self {
        Self("hello\n")
    }
}

static DIGEST_KEY: Key = Key::from_bytes([7; 32]);
static NO_STORE: Remembered = Remembered::EMPTY;

fn secrets() -> Secrets<'static> {
    Secrets::new(&DIGEST_KEY, &NO_STORE)
}

/// One file, declared.
fn declared() -> Graph {
    let mut graph = Graph::new();
    graph.preexisting(Target::new("web-01").unwrap());
    graph
        .insert(
            Resource::new(address("os:file.motd@web-01"))
                .with("path", Value::Text("/etc/motd".to_owned()))
                .with("content", Value::Text("hello\n".to_owned())),
        )
        .unwrap();
    graph
}

/// State holding one entry for that file: a change of `kind` that ran and was
/// never recorded, whose provider says reading cannot settle it.
fn entry(kind: ChangeKind) -> State {
    standing(kind, Standing::Pending)
}

/// The same, where it stands is said.
fn standing(kind: ChangeKind, stands: Standing) -> State {
    let mut state = State::new();
    let mut locator = Attributes::new();
    locator.insert("path".to_owned(), Value::Text("/etc/motd".to_owned()));
    state.mark(
        address("os:file.motd@web-01"),
        Mark::new(kind, locator, Attributes::new())
            .standing(stands)
            .written_at(Written::at(1_700_000_000)),
    );
    state
}

/// Plans against a machine holding a file with `content`.
fn planned(graph: &Graph, state: &State, content: &'static str) -> Plan {
    let providers = Providers::new().with(Box::new(Holding(content)));
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

fn only(plan: &Plan) -> &Change {
    let changes: Vec<&Change> = plan.changes().collect();
    assert_eq!(
        changes.len(),
        1,
        "one change: {:?}",
        changes
            .iter()
            .map(|change| format!("{} {:?}", change.address(), change.kind()))
            .collect::<Vec<_>>()
    );
    changes[0]
}

/// The reading matches the declaration, so the declaration's own change would
/// send nothing — and what goes instead is the entry's kind, again.
///
/// Adopted, it would be written down as a thing this run never touched and
/// nobody knows the state of. The whole point of the entry is that reading
/// cannot tell whether the change took, so the answer is to do it again.
#[test]
fn an_entry_still_declared_and_matching_is_sent_again() {
    let plan = planned(&declared(), &entry(ChangeKind::Update), "hello\n");
    let change = only(&plan);

    assert_eq!(
        change.kind(),
        ChangeKind::Update,
        "the entry's own kind goes again, not an adoption"
    );
    assert!(change.is_resent(), "and it says it is being sent again");
}

/// And where the reading does not match, the declaration's change goes as it
/// always would — it was going to send something anyway, and it is worked out
/// from what is there now rather than from what a stopped run meant to do.
#[test]
fn an_entry_whose_reading_does_not_match_goes_as_the_declaration_says() {
    let plan = planned(&declared(), &entry(ChangeKind::Update), "goodbye\n");
    let change = only(&plan);

    assert_eq!(change.kind(), ChangeKind::Update);
    assert_eq!(
        change.deltas().len(),
        1,
        "worked out from the reading: the content differs"
    );
    assert!(
        !change.is_resent(),
        "nothing is repeated; this change was decided afresh"
    );
}

/// A replacement that got past its forget point had its removal half done: what
/// was there is gone, and the record with it. What is left to repeat is the
/// making of the new one.
#[test]
fn an_entry_past_its_forget_point_is_sent_again_as_a_create() {
    let mut state = State::new();
    let mut locator = Attributes::new();
    locator.insert("path".to_owned(), Value::Text("/etc/motd".to_owned()));
    state.mark(
        address("os:file.motd@web-01"),
        Mark::new(ChangeKind::Replace, locator, Attributes::new())
            .past_forgetting()
            .standing(Standing::Pending)
            .written_at(Written::at(1_700_000_000)),
    );

    let plan = planned(&declared(), &state, "hello\n");
    let change = only(&plan);

    assert_eq!(
        change.kind(),
        ChangeKind::Create,
        "the removal half is done; the create half is what is repeated"
    );
    assert!(change.is_resent());
    assert!(
        change.is_past_forgetting(),
        "and a reader is told which half of a replacement this is"
    );
}

/// A removal that was marked and never recorded, whose declaration is back.
///
/// The removal is never repeated: somebody put the declaration back, and
/// repeating the delete would take away what they asked for. What goes is the
/// declaration.
#[test]
fn an_entry_for_a_removal_whose_declaration_came_back_is_a_create() {
    let plan = planned(&declared(), &entry(ChangeKind::Delete), "hello\n");
    let change = only(&plan);

    assert_eq!(
        change.kind(),
        ChangeKind::Create,
        "what is declared is made, never the removal a second time"
    );
    assert!(change.is_resent());
}

/// An entry kept only so that what a failed change made could be removed is not
/// sent again.
///
/// Its provider said reading settles this change, and the reading says the
/// declaration is satisfied. That is an answer, not a doubt: what it gets is a
/// record, the same as anything else found as declared. Only an entry reading
/// cannot settle is repeated.
#[test]
fn an_entry_kept_for_removal_only_is_not_sent_again() {
    let state = standing(ChangeKind::Update, Standing::ForRemovalOnly);
    let plan = planned(&declared(), &state, "hello\n");
    let change = only(&plan);

    assert_eq!(
        change.kind(),
        ChangeKind::Adopt,
        "the reading settles it, so it is written down rather than done again"
    );
    assert!(!change.is_resent());
}

/// Where there is no record, a resource is looked for where its entry says.
///
/// A change that failed after its operations ran may have made something, and
/// the entry is the only thing that knows where. Looked for where the
/// declaration now points instead, what that change made is never seen again:
/// the plan makes a second one somewhere else, and the first is left on the
/// machine with nothing naming it. RFC 0001 §5.6.
#[test]
fn what_has_no_record_is_looked_for_where_its_entry_says() {
    // Declared somewhere other than where the entry remembers it.
    let mut graph = Graph::new();
    graph.preexisting(Target::new("web-01").unwrap());
    graph
        .insert(
            Resource::new(address("os:file.motd@web-01"))
                .with("path", Value::Text("/etc/issue".to_owned()))
                .with("content", Value::Text("hello\n".to_owned())),
        )
        .unwrap();

    let plan = planned(&graph, &entry(ChangeKind::Create), "hello\n");
    let change = only(&plan);

    assert!(
        !change.is_resent(),
        "the reading does not match the declaration: it is at the old path"
    );
    let moved: Vec<(&str, String)> = change
        .deltas()
        .iter()
        .map(|delta| {
            (
                delta.name(),
                delta
                    .before()
                    .map_or_else(|| "—".to_owned(), ToString::to_string),
            )
        })
        .collect();
    assert_eq!(
        moved,
        // Quoted the way a plan shows text it read off a machine.
        vec![("path", "\"/etc/motd\"".to_owned())],
        "it was found where the entry said, and moving it is the work"
    );
}
