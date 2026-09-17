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
use nerpa_core::planner::plan;
use nerpa_core::provider::{
    Afterwards, Answered, Attributes, Concluded, Concluding, Machines, Planned, Prepared, Provider,
    ProviderError, Providers, Reach, Read, Reading, Sending,
};
use nerpa_core::run::apply;
use nerpa_core::schema::Schema;
use nerpa_core::secret::{Key, Recorded, Remembered, Secrets};
use nerpa_core::state::{Record, State};
use nerpa_core::{Address, Graph, Observed, Resource, Value};
use nerpa_proto::{Answer, Ask, Entry, Operation, Seen};

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
#[derive(Debug)]
struct Telling(Ledger);

impl Reach for Telling {
    fn ask(&self, ask: &Ask) -> Result<Answer, ProviderError> {
        match ask {
            Ask::Look { observations, .. } => Ok(Answer::Found {
                seen: observations.iter().map(|_| Seen::Nothing).collect(),
            }),
            Ask::Perform { operations, .. } => {
                self.0.note(format!("sent {}", said(operations)));
                Ok(Answer::Done {
                    became: operations
                        .iter()
                        .map(|operation| match operation {
                            Operation::File { content, mode, .. } => Seen::Entry(Entry::File {
                                content: content.clone(),
                                mode: *mode,
                            }),
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
struct Writes;

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
            });
        }
        Ok(Planned {
            operations,
            forget_after,
            carries: BTreeMap::new(),
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

    fn apply(
        &self,
        _address: &Address,
        _kind: nerpa_core::ChangeKind,
        _roused: bool,
        attributes: &Attributes,
        _premise: &[nerpa_proto::Premise],
        _facts: nerpa_core::observed::Facts<'_>,
        _reach: &dyn Reach,
    ) -> Result<Attributes, ProviderError> {
        Ok(attributes.clone())
    }

    fn delete(
        &self,
        _address: &Address,
        _recorded: &Attributes,
        _premise: &[nerpa_proto::Premise],
        _facts: nerpa_core::observed::Facts<'_>,
        _reach: &dyn Reach,
    ) -> Result<(), ProviderError> {
        Ok(())
    }

    fn fake() -> Self {
        Self
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
fn run(graph: &Graph, observed: &Observed, mut state: State) -> (Ledger, State) {
    let ledger = Ledger::default();
    let providers = Providers::new().with(Box::new(Writes));
    let machines = Machines::new().everywhere(Box::new(Telling(ledger.clone())));
    let intended =
        plan(graph, observed, &state, &providers.schema(), secrets()).expect("plannable");

    let watching = ledger.clone();
    let mut record = move |state: &State| {
        watching.note(if state.get(&address(MOTD)).is_some() {
            "state: recorded".to_owned()
        } else {
            "state: nothing".to_owned()
        });
        Ok(())
    };
    apply(
        &intended,
        &providers,
        &machines,
        observed,
        &mut state,
        secrets(),
        &mut record,
    )
    .expect("applied");
    (ledger, state)
}

#[test]
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
            "sent remove /etc/motd.old".to_owned(),
            "state: nothing".to_owned(),
            "sent write /etc/motd".to_owned(),
            "state: recorded".to_owned(),
        ],
        "the old one goes, the record goes with it, and only then is the new one made"
    );
    assert!(state.get(&address(MOTD)).is_some());
}
