//! Who speaks to a machine when a change is applied.
//!
//! A provider says what a change is made of; the engine sends it. The difference
//! shows in the order things happen: one batch per change, and a replacement's
//! record dropped between the half that removes and the half that makes.
//! RFC 0001 §5.2.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use nerpa_core::address::address;
use nerpa_core::plan::ChangeKind;

use nerpa_core::provider::{
    Afterwards, Answered, Attributes, Concluded, Concluding, Machines, Planned, Prepared, Provider,
    ProviderError, Providers, Reach, Read, Reading, Sending,
};
use nerpa_core::run::{RunError, apply};
use nerpa_core::schema::Schema;
use nerpa_core::secret::{Key, Recorded, Remembered, Secrets, Source};
use nerpa_core::state::{Mark, Record, Standing, State};
use nerpa_core::{Address, Graph, Observed, Resource, Value};
use nerpa_proto::{Answer, Ask, At, Entry, Operation, Seen};

/// The plan a test drives, attributed to a tester: `run::apply` proceeds only
/// with an actor identity (`INV-TRUST-001`), and a test is a run of a kind.
fn plan(
    graph: &Graph,
    observed: &Observed,
    state: &State,
    schema: &Schema,
    secrets: Secrets<'_>,
) -> Result<nerpa_core::Plan, nerpa_core::planner::PlanError> {
    nerpa_core::planner::plan(graph, observed, state, schema, secrets).map(|planned| {
        planned.attributed_to(
            nerpa_core::Actor::new("tester", nerpa_core::Grant::default(), None).expect("an actor"),
        )
    })
}

static DIGEST_KEY: Key = Key::from_bytes([9; 32]);
static NO_STORE: Remembered = Remembered::EMPTY;

fn secrets() -> Secrets<'static> {
    Secrets::new(&DIGEST_KEY, &NO_STORE)
}

/// What happened, in the order it happened: what was sent to the machine, and
/// what state said each time it was written.
#[derive(Debug, Default, Clone)]
struct Ledger(Arc<Mutex<Vec<String>>>);

impl Ledger {
    fn note(&self, what: String) {
        self.0.lock().unwrap().push(what);
    }

    fn read(&self) -> Vec<String> {
        self.0.lock().unwrap().clone()
    }
}

/// A machine that answers every operation as though it had worked, and writes
/// down what it was asked to do.
///
/// `truthfully` is whether its digests describe what it was sent. A machine that
/// answers with a digest of something else is the shape of a write that did not
/// take, or of a far end that is not doing what it says.
#[derive(Debug)]
struct Telling {
    ledger: Ledger,
    truthfully: bool,
    /// What it answers a batch with instead of doing it, if anything.
    refusing: Option<Answer>,
}

impl Reach for Telling {
    fn ask(&self, ask: &Ask) -> Result<Answer, ProviderError> {
        match ask {
            Ask::Look { observations, .. } => Ok(Answer::Found {
                seen: observations.iter().map(|_| Seen::Nothing).collect(),
            }),
            Ask::Perform { operations, .. } => {
                self.ledger.note(format!("sent {}", said(operations)));
                if let Some(refusal) = &self.refusing {
                    return Ok(refusal.clone());
                }
                Ok(Answer::Done {
                    became: operations
                        .iter()
                        .map(|operation| match operation {
                            Operation::File { content, mode, .. } => Seen::Digest {
                                digest: nerpa_proto::digest(&Seen::Entry(Entry::File {
                                    content: if self.truthfully {
                                        content.clone()
                                    } else {
                                        b"something else entirely\n".to_vec()
                                    },
                                    mode: *mode,
                                })),
                            },
                            _ => Seen::Nothing,
                        })
                        .collect(),
                })
            }
            _ => Err(ProviderError::NotOnAMachine),
        }
    }
}

/// One line for a batch, naming what each of its operations does.
fn said(operations: &[Operation]) -> String {
    operations
        .iter()
        .map(|operation| match operation {
            Operation::File { path, .. } => format!("write {path}"),
            Operation::Remove { path } => format!("remove {path}"),
            other => format!("{other:?}"),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Writes files, and reaches nothing: it is handed no connection to reach with.
#[derive(Debug)]
struct Writes {
    /// Whether its write says it carries what it was given. A write that
    /// carries nothing confirms nothing.
    carrying: bool,
}

impl Provider for Writes {
    fn name(&self) -> &nerpa_core::Provider {
        static NAME: std::sync::OnceLock<nerpa_core::Provider> = std::sync::OnceLock::new();
        NAME.get_or_init(|| nerpa_core::Provider::new("os").unwrap())
    }

    fn schema(&self) -> Schema {
        Schema::new().immutable(
            nerpa_core::Provider::new("os").unwrap(),
            nerpa_core::Kind::new("file").unwrap(),
            "path",
        )
    }

    fn read(
        &self,
        _batch: &[Reading],
        _facts: nerpa_core::observed::Facts<'_>,
        _reach: &dyn Reach,
    ) -> Result<Read, ProviderError> {
        Ok(Read::new())
    }

    fn prepare(&self, change: &Prepared<'_>) -> Result<Planned, ProviderError> {
        let path = |attributes: &Attributes| match attributes.get("path") {
            Some(Value::Text(path)) => path.clone(),
            _ => panic!("every file in these tests has a path"),
        };

        let mut operations = Vec::new();
        let mut forget_after = None;
        if matches!(change.kind, Sending::Replace | Sending::Delete)
            && let Some(recorded) = change.recorded
        {
            operations.push(Operation::Remove {
                path: path(recorded),
            });
            if change.kind == Sending::Replace {
                forget_after = Some(operations.len());
            }
        }
        if change.kind != Sending::Delete {
            operations.push(Operation::File {
                path: path(change.desired),
                content: b"hello\n".to_vec(),
                mode: 0o644,
                secret: false,
            });
        }
        let mut carries = BTreeMap::new();
        if self.carrying && change.kind != Sending::Delete {
            carries.insert(
                operations.len() - 1,
                ["content".to_owned(), "path".to_owned()]
                    .into_iter()
                    .collect::<BTreeSet<_>>(),
            );
        }
        Ok(Planned {
            operations,
            forget_after,
            carries,
        })
    }

    fn conclude(
        &self,
        change: &Concluding<'_>,
        _became: &[Answered],
    ) -> Result<Concluded, ProviderError> {
        if change.kind == Sending::Delete {
            return Ok(Concluded::Gone);
        }
        Ok(Concluded::Became(change.plain.clone()))
    }

    fn afterwards(&self, _address: &Address, _kind: Sending) -> Afterwards {
        Afterwards::Read
    }

    fn fake() -> Self {
        Self { carrying: false }
    }
}

const MOTD: &str = "os:file.motd@web-01";

fn graph_of(path: &str) -> Graph {
    let mut graph = Graph::new();
    graph.preexisting(address(MOTD).target().unwrap().clone());
    graph
        .insert(Resource::new(address(MOTD)).with("path", Value::Text(path.to_owned())))
        .unwrap();
    graph
}

/// Runs the plan the graph asks for, against a machine that writes down what it
/// was asked, and a recorder that writes down what state said.
fn run(graph: &Graph, observed: &Observed, state: State) -> (Ledger, State) {
    let (ledger, state, outcome) = attempt(
        graph,
        observed,
        state,
        secrets(),
        Writes { carrying: false },
    );
    outcome.expect("applied");
    (ledger, state)
}

/// The same, for a run that is expected to refuse.
fn attempt(
    graph: &Graph,
    observed: &Observed,
    mut state: State,
    secrets: Secrets<'_>,
    provider: Writes,
) -> (Ledger, State, Result<(), RunError>) {
    let ledger = Ledger::default();
    let providers = Providers::new().with(Box::new(provider));
    let machines = Machines::new().everywhere(Box::new(Telling {
        ledger: ledger.clone(),
        truthfully: true,
        refusing: None,
    }));
    let intended = plan(graph, observed, &state, &providers.schema(), secrets).expect("plannable");

    let watching = ledger.clone();
    let mut record = move |state: &State| {
        // A mark first, because a write that carries one is a write made before
        // the request it belongs to went out.
        watching.note(if state.marked(&address(MOTD)).is_some() {
            "state: marked".to_owned()
        } else if state.get(&address(MOTD)).is_some() {
            "state: recorded".to_owned()
        } else {
            "state: nothing".to_owned()
        });
        Ok(())
    };
    let outcome = apply(
        &intended,
        &providers,
        &machines,
        observed,
        &mut state,
        secrets,
        &mut record,
    )
    .map(|_| ());
    (ledger, state, outcome)
}

#[test]
#[dacc_derive::doc_anchor(id = "the-engine-sends-what-the-provider-prepared")]
fn the_engine_sends_what_the_provider_prepared() {
    let (ledger, state) = run(&graph_of("/etc/motd"), &Observed::new(), State::new());

    assert_eq!(
        ledger
            .read()
            .iter()
            .filter(|line| line.starts_with("sent "))
            .collect::<Vec<_>>(),
        vec!["sent write /etc/motd"],
        "one batch for the one change: {:?}",
        ledger.read()
    );
    assert!(state.get(&address(MOTD)).is_some());
}

#[test]
#[dacc_derive::doc_anchor(
    id = "a-replacement-is-forgotten-between-the-half-that-removes-and-the-half-that-makes"
)]
fn a_replacement_is_forgotten_between_the_half_that_removes_and_the_half_that_makes() {
    let mut was = Attributes::new();
    was.insert("path".to_owned(), Value::Text("/etc/motd.old".to_owned()));
    let mut observed = Observed::new();
    observed.record(address(MOTD), was.clone());

    let recorded: BTreeMap<String, Recorded> = was
        .iter()
        .map(|(name, value)| (name.clone(), Recorded::Plain(value.clone())))
        .collect();
    let mut state = State::new();
    state.record(
        address(MOTD),
        Record::new(recorded.into_iter().collect(), BTreeSet::new()),
    );

    let (ledger, state) = run(&graph_of("/etc/motd"), &observed, state);

    assert_eq!(
        ledger.read(),
        vec![
            // Written before anything is sent: from here on, a run that stops
            // leaves something saying this machine was being acted on.
            "state: marked".to_owned(),
            "sent remove /etc/motd.old".to_owned(),
            "state: marked".to_owned(),
            "sent write /etc/motd".to_owned(),
            "state: recorded".to_owned(),
        ],
        "the mark comes first, then the old one goes, then the new one is made"
    );
    assert!(state.get(&address(MOTD)).is_some());
}

/// A secret is carried by the write that sets it, or the change does not go.
///
/// What a provider says a write carries is what a matching answer confirms. A
/// secret no write carries would be sent and then confirmed by nothing, and
/// state would record a digest of a value nobody checked arrived. RFC 0001 §5.2.
/// One file whose contents are a secret held in the store.
fn graph_of_a_secret() -> Graph {
    let mut graph = Graph::new();
    graph.preexisting(address(MOTD).target().unwrap().clone());
    graph
        .insert(
            Resource::new(address(MOTD))
                .with("path", Value::Text("/etc/motd".to_owned()))
                .with(
                    "content",
                    Source::Stored {
                        provider: "remembered".to_owned(),
                        path: "db/password".to_owned(),
                        version: None,
                    },
                ),
        )
        .unwrap();
    graph
}

#[test]
fn a_secret_no_write_carries_is_refused_before_anything_is_sent() {
    let store = Remembered::new().holding("db/password", "correct-horse");
    let secrets = Secrets::new(&DIGEST_KEY, &store);

    let (ledger, _, outcome) = attempt(
        &graph_of_a_secret(),
        &Observed::new(),
        State::new(),
        secrets,
        Writes { carrying: false },
    );

    let Err(RunError::Uncarried { attribute, .. }) = outcome else {
        panic!("a secret nothing carries is not a change that can be sent: {outcome:?}");
    };
    assert_eq!(attribute, "content");
    assert!(
        ledger.read().iter().all(|line| !line.starts_with("sent ")),
        "and it is refused before the machine is asked anything: {:?}",
        ledger.read()
    );
}

/// Carrying a secret is not the same as saying so.
///
/// The write that sets a secret is marked secret on the wire, because that is
/// what makes the far end answer with a sealed digest rather than with what it
/// read back — and an unmarked write would have the machine answer with the
/// password itself. The provider marks it and the engine checks the mark, so a
/// provider that forgets is caught by something that did not write the mark.
#[test]
#[dacc_derive::doc_anchor(id = "a-secret-carried-by-a-write-that-is-not-marked-secret-is-refused")]
fn a_secret_carried_by_a_write_that_is_not_marked_secret_is_refused() {
    let store = Remembered::new().holding("db/password", "correct-horse");
    let secrets = Secrets::new(&DIGEST_KEY, &store);

    let (ledger, _, outcome) = attempt(
        &graph_of_a_secret(),
        &Observed::new(),
        State::new(),
        secrets,
        Writes { carrying: true },
    );

    let Err(RunError::Unflagged { attribute, .. }) = outcome else {
        panic!("a write that carries a secret and does not say so is not sent: {outcome:?}");
    };
    assert_eq!(attribute, "content");
    assert!(
        ledger.read().iter().all(|line| !line.starts_with("sent ")),
        "and it is refused before the machine is asked anything: {:?}",
        ledger.read()
    );
}

/// A write whose answer is not what it was sent stops the run.
///
/// The engine knows what the path must hash to, because it holds what it sent.
/// An answer that is anything else means the path holds something other than
/// what was written there — and recording it as written would be a record of
/// something nobody made, which no later reading can correct because the record
/// would look exactly like the truth.
#[test]
#[dacc_derive::doc_anchor(id = "a-write-answered-with-the-wrong-digest-is-not-recorded-as-written")]
fn a_write_answered_with_the_wrong_digest_is_not_recorded_as_written() {
    let ledger = Ledger::default();
    let providers = Providers::new().with(Box::new(Writes { carrying: false }));
    let machines = Machines::new().everywhere(Box::new(Telling {
        ledger: ledger.clone(),
        truthfully: false,
        refusing: None,
    }));
    let graph = graph_of("/etc/motd");
    let observed = Observed::new();
    let mut state = State::new();
    let intended =
        plan(&graph, &observed, &state, &providers.schema(), secrets()).expect("plannable");

    let outcome = apply(
        &intended,
        &providers,
        &machines,
        &observed,
        &mut state,
        secrets(),
        &mut |_: &State| Ok(()),
    );

    let Err(RunError::Unconfirmed { at, .. }) = outcome else {
        panic!("an answer that is not what was sent is not a write that took: {outcome:?}");
    };
    assert_eq!(at, 0);
    assert!(
        state.get(&address(MOTD)).is_none(),
        "and nothing about it is recorded"
    );
}

/// A change is written down as being sent before it is sent.
///
/// The write that carries the mark is the one that happens first: a run that
/// stops between it and the answer leaves a machine that was acted on and a
/// state file saying so, rather than one that says nothing happened. RFC 0001
/// §5.6.
#[test]
fn a_change_is_marked_before_it_is_sent() {
    let (ledger, _) = run(&graph_of("/etc/motd"), &Observed::new(), State::new());

    let happened = ledger.read();
    assert_eq!(
        happened.first().map(String::as_str),
        Some("state: marked"),
        "the mark is written before anything else happens: {happened:?}"
    );
    let marked = happened.iter().position(|line| line == "state: marked");
    let sent = happened.iter().position(|line| line.starts_with("sent "));
    assert!(
        marked < sent,
        "and before the request goes out: {happened:?}"
    );
}

/// And the write that records what it became takes the mark away.
///
/// A mark left behind would hold its machine for ever: what holds one is a
/// change whose outcome nobody knows, and this one is known.
#[test]
fn a_change_that_was_recorded_leaves_no_mark() {
    let (ledger, state) = run(&graph_of("/etc/motd"), &Observed::new(), State::new());

    assert!(
        state.marked(&address(MOTD)).is_none(),
        "the answer settled it: {:?}",
        ledger.read()
    );
    assert!(state.get(&address(MOTD)).is_some(), "and it was recorded");
    assert_eq!(
        ledger.read().last().map(String::as_str),
        Some("state: recorded"),
        "the last write says so: {:?}",
        ledger.read()
    );
}

/// A run against a machine that refuses the batch in the way the test asks for.
fn refused(refusal: Answer) -> (State, Result<(), RunError>) {
    let ledger = Ledger::default();
    let providers = Providers::new().with(Box::new(Writes { carrying: false }));
    let machines = Machines::new().everywhere(Box::new(Telling {
        ledger,
        truthfully: true,
        refusing: Some(refusal),
    }));
    let graph = graph_of("/etc/motd");
    let observed = Observed::new();
    let mut state = State::new();
    let intended =
        plan(&graph, &observed, &state, &providers.schema(), secrets()).expect("plannable");

    let outcome = apply(
        &intended,
        &providers,
        &machines,
        &observed,
        &mut state,
        secrets(),
        &mut |_: &State| Ok(()),
    )
    .map(|_| ());
    (state, outcome)
}

/// A change that failed after its operations ran leaves an entry.
///
/// Something happened on that machine and nobody knows what: the operations ran
/// and the answer says one of them stopped part way. The mark stops standing for
/// a request in flight and becomes what the next run has to deal with. RFC 0001
/// §5.6.
#[test]
fn a_change_that_failed_after_its_operations_leaves_an_entry() {
    let (state, outcome) = refused(Answer::Failed {
        at: At::Operation(0),
        problem: "no room on the device".to_owned(),
    });

    assert!(outcome.is_err(), "the change did not take");
    let mark = state
        .marked(&address(MOTD))
        .expect("something of it is left behind");
    assert!(
        !mark.in_flight(),
        "nothing is in flight any more: the answer came back"
    );
    assert_eq!(
        mark.stands(),
        Standing::ForRemovalOnly,
        "this provider says reading resolves it, so the entry is kept only to \
         remove by"
    );
}

/// And a change that failed before any of them ran leaves nothing.
///
/// The machine is not what the plan was computed from, so nothing was done to
/// it. A mark left standing would say a request went out, and none did.
#[test]
fn a_change_that_failed_before_its_operations_leaves_nothing() {
    let (state, outcome) = refused(Answer::Moved { at: 0 });

    assert!(outcome.is_err(), "the plan is stale");
    assert!(
        state.marked(&address(MOTD)).is_none(),
        "nothing ran, so nothing is left to deal with"
    );
}

/// What an entry remembers is removed at the locator the entry carries.
///
/// A change that failed after its operations ran left no record — a record is
/// written from what it became — so nothing else knows where to look. Without
/// this the thing it may have made stays on the machine with nothing recording
/// it. RFC 0001 §5.6.
#[test]
fn what_an_entry_remembers_is_removed_at_its_locator() {
    let mut locator = Attributes::new();
    locator.insert("path".to_owned(), Value::Text("/etc/motd".to_owned()));
    let mut state = State::new();
    state.mark(
        address(MOTD),
        Mark::new(ChangeKind::Create, locator, Attributes::new())
            .standing(Standing::ForRemovalOnly),
    );

    // Nothing is declared any more.
    let mut graph = Graph::new();
    graph.preexisting(address(MOTD).target().unwrap().clone());
    let (ledger, state) = run(&graph, &Observed::new(), state);

    assert!(
        ledger
            .read()
            .iter()
            .any(|line| line == "sent remove /etc/motd"),
        "it was removed where the entry said it would be: {:?}",
        ledger.read()
    );
    assert!(
        state.marked(&address(MOTD)).is_none(),
        "and the entry went with the removal"
    );
}

/// And an entry kept only for removal goes when the reading is written down.
///
/// Its provider said reading could settle this change, and the reading says the
/// declaration is satisfied — so the record written from what was read is the
/// answer the entry was waiting for. Left behind, it would outlive the question
/// it was asked, and the machine would carry an entry nothing can ever clear.
/// RFC 0001 §5.6.
#[test]
fn a_record_written_from_the_reading_clears_an_entry_kept_for_removal() {
    let mut locator = Attributes::new();
    locator.insert("path".to_owned(), Value::Text("/etc/motd".to_owned()));
    let mut state = State::new();
    state.mark(
        address(MOTD),
        Mark::new(ChangeKind::Create, locator, Attributes::new())
            .standing(Standing::ForRemovalOnly),
    );

    // The machine has it, exactly as declared: nothing is sent.
    let mut observed = Observed::new();
    observed.record(
        address(MOTD),
        [("path".to_owned(), Value::Text("/etc/motd".to_owned()))]
            .into_iter()
            .collect(),
    );
    let (ledger, state) = run(&graph_of("/etc/motd"), &observed, state);

    assert!(
        !ledger.read().iter().any(|line| line.starts_with("sent ")),
        "nothing was sent: {:?}",
        ledger.read()
    );
    assert!(
        state.get(&address(MOTD)).is_some(),
        "what was read is written down"
    );
    assert!(
        state.marked(&address(MOTD)).is_none(),
        "and the entry it was standing on is cleared: {:?}",
        state.marked(&address(MOTD))
    );
}
